
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { hashHistory } from 'react-router'
import ReactGA from 'react-ga'

import Link from '../components/Link';
import TrackList from '../components/TrackList'
import Thumbnail from '../components/Thumbnail'
import Parallax from '../components/Parallax'
import Dater from '../components/Dater'
import ConfirmationButton from '../components/Fields/ConfirmationButton'
import LazyLoadListener from '../components/LazyLoadListener'
import FollowButton from '../components/Fields/FollowButton'
import Header from '../components/Header'
import ContextMenuTrigger from '../components/ContextMenuTrigger'
import URILink from '../components/URILink'
import Icon from '../components/Icon'

import * as helpers from '../helpers'
import * as coreActions from '../services/core/actions'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Playlist extends React.Component{

	constructor(props){
		super(props);
	}

	componentWillMount(){
		var uri = this.props.params.uri;

		// Spotify upgraded their playlists URI to remove user component (Sept 2018)
		// We accept the old format, and redirect to the new one
		if (uri.includes("spotify:user:")){
			uri = uri.replace(/spotify:user:([^:]*?):/i, "spotify:");
			hashHistory.push(global.baseURL+'playlist/'+encodeURIComponent(uri));
		}
	}

	componentDidMount(){
		this.setWindowTitle();
		this.props.coreActions.loadPlaylist(this.props.params.uri);
	}

	componentWillReceiveProps(nextProps){
		if (nextProps.params.uri != this.props.params.uri){
			this.props.coreActions.loadPlaylist(nextProps.params.uri);
		} else if (!this.props.mopidy_connected && nextProps.mopidy_connected){
			if (helpers.uriSource(this.props.params.uri) != 'spotify'){
				this.props.coreActions.loadPlaylist(nextProps.params.uri);
			}
		}

		if (!this.props.playlist && nextProps.playlist){
			this.setWindowTitle(nextProps.playlist);
		}

		if (this.props.params.uri !== nextProps.params.uri && nextProps.playlist){
			this.setWindowTitle(nextProps.playlist);
		}
	}

	setWindowTitle(playlist = this.props.playlist){		
		if (playlist){
			this.props.uiActions.setWindowTitle(playlist.name+" (playlist)");
		} else{
			this.props.uiActions.setWindowTitle("Playlist");
		}
	}

	loadMore(){
		this.props.spotifyActions.getMore(
			this.props.playlist.tracks_more,
			{
				parent_type: 'playlist',
				parent_key: this.props.playlist.uri,
				records_type: 'track'
			}
		);
	}

	handleContextMenu(e){
		var data = {
			e: e,
			context: (this.props.playlist.can_edit ? 'editable-playlist' : 'playlist'),
			items: [this.props.playlist],
			uris: [this.props.params.uri]
		}
		this.props.uiActions.showContextMenu(data)
	}

	play(){
        this.props.mopidyActions.playPlaylist(this.props.playlist.uri)
	}

	follow(){
        if (this.props.allow_reporting){
	        ReactGA.event({ category: 'Playlist', action: 'Follow', label: this.props.playlist.uri });
	    }
		this.props.spotifyActions.toggleFollowingPlaylist(this.props.playlist.uri, 'PUT')
	}

	// TODO: Once unfollowing occurs, remove playlist from global playlists list
	unfollow(){
        if (this.props.allow_reporting){
	        ReactGA.event({ category: 'Playlist', action: 'Unfollow', label: this.props.playlist.uri });
	    }
		this.props.spotifyActions.toggleFollowingPlaylist(this.props.playlist.uri, 'DELETE' )
	}

	// TODO: Once deletion occurs, remove playlist from global playlists list
	delete(){
		this.props.mopidyActions.deletePlaylist(this.props.playlist.uri);
	}

	reorderTracks(indexes, index){
		this.props.coreActions.reorderPlaylistTracks(this.props.playlist.uri, indexes, index, this.props.playlist.snapshot_id);
	}

	removeTracks(tracks_indexes){
		this.props.coreActions.removeTracksFromPlaylist(this.props.playlist.uri, tracks_indexes);
	}

	inLibrary(){
		var library = helpers.uriSource(this.props.params.uri)+'_library_playlists';
		return (this.props[library] && this.props[library].indexOf(this.props.params.uri) > -1);
	}

	renderActions(){
		switch(helpers.uriSource(this.props.playlist.uri)){

			case 'm3u':
				return (
					<div className="actions">
						<button className="primary" onClick={ e => this.play() }>Play</button>
						<Link className="button secondary" to={global.baseURL+'playlist/'+encodeURIComponent(this.props.params.uri)+'/edit'}>Edit</Link>
						<ContextMenuTrigger onTrigger={e => this.handleContextMenu(e)} />
					</div>
				)

			case 'spotify':
				if (this.props.playlist.can_edit){
					return (
						<div className="actions">
							<button className="primary" onClick={ e => this.play() }>Play</button>
							<Link className="button secondary" to={global.baseURL+'playlist/'+encodeURIComponent(this.props.params.uri)+'/edit'}>Edit</Link>
							<ContextMenuTrigger onTrigger={e => this.handleContextMenu(e)} />
						</div>
					)
				}
				return (
					<div className="actions">
						<button className="primary" onClick={ e => this.play() }>Play</button>
						<FollowButton className="secondary" uri={this.props.params.uri} addText="Add to library" removeText="Remove from library" is_following={this.inLibrary()} />
						<ContextMenuTrigger onTrigger={e => this.handleContextMenu(e)} />
					</div>
				)

			default:
				return (
					<div className="actions">
						<button className="primary" onClick={ e => this.play() }>Play</button>
						<ContextMenuTrigger onTrigger={e => this.handleContextMenu(e)} />
					</div>
				)
		}
	}

	render(){
		var scheme = helpers.uriSource(this.props.params.uri);
		var playlist_id = helpers.getFromUri('playlistid',this.props.params.uri);
		
		if (!this.props.playlist){
			if (helpers.isLoading(this.props.load_queue,['spotify_playlists/'+playlist_id+'?'])){
				return (
					<div className="body-loader loading">
						<div className="loader"></div>
					</div>
				)
			} else {
				return null;
			}
		}

		var playlist = helpers.collate(this.props.playlist, {tracks: this.props.tracks, users: this.props.users});

		var context = 'playlist';
		if (playlist.can_edit){
			context = 'editable-playlist';
		}

		if (!playlist.tracks_uris || (playlist.tracks_uris && !playlist.tracks) || (playlist.tracks_uris.length !== playlist.tracks.length)){
			var is_loading_tracks = true;
		} else {
			var is_loading_tracks = false;
		}

		return (
			<div className="view playlist-view content-wrapper">

				<Parallax image={playlist.images ? playlist.images.huge : null} blur />
				
				<div className="thumbnail-wrapper">
					<Thumbnail size="large" canZoom images={playlist.images} />
				</div>

				<div className="title">
					<h1>{playlist.name}</h1>
					{playlist.description ? <h2 className="description mid_grey-text" dangerouslySetInnerHTML={{__html: playlist.description}}></h2> : null }

					<ul className="details">
						{!this.props.slim_mode ? <li className="tooltip"><Icon type="fontawesome" name={helpers.sourceIcon(playlist.uri)} /><span className="tooltip__content">{helpers.uriSource(playlist.uri)} playlist</span></li> : null }
						{playlist.user_uri ? <li><URILink type="user" uri={playlist.user_uri}>{playlist.user ? playlist.user.name : helpers.getFromUri('userid',playlist.user_uri)}</URILink></li> : null }
						{!this.props.slim_mode && playlist.followers !== undefined ? <li>{playlist.followers.toLocaleString()} followers</li> : null }
						{!this.props.slim_mode && playlist.last_modified_date ? <li>Edited <Dater type="ago" data={playlist.last_modified_date} /></li> : null }
						<li>
							{playlist.tracks_total ? playlist.tracks_total : (playlist.tracks ? playlist.tracks.length : '0')} tracks,&nbsp;
							<Dater type="total-time" data={playlist.tracks} />
						</li>
					</ul>
				</div>

				{this.renderActions()}

				<section className="list-wrapper">
					<TrackList uri={playlist.uri} className="playlist-track-list" context={context} tracks={playlist.tracks} removeTracks={ tracks_indexes => this.removeTracks(tracks_indexes) } reorderTracks={ (indexes, index) => this.reorderTracks(indexes, index) } />
					<LazyLoadListener
						loadKey={playlist.tracks_more}
						showLoader={is_loading_tracks || playlist.tracks_more}
						loadMore={() => this.loadMore()}
					/>
				</section>
			</div>
		)
	}
}


/**
 * Export our component
 *
 * We also integrate our global store, using connect()
 **/

const mapStateToProps = (state, ownProps) => {
	var uri = ownProps.params.uri;
	return {
		allow_reporting: state.ui.allow_reporting,
		slim_mode: state.ui.slim_mode,
		load_queue: state.ui.load_queue,
		users: state.core.users,
		tracks: state.core.tracks,
		playlist: (state.core.playlists[uri] !== undefined ? state.core.playlists[uri] : false ),
		spotify_library_playlists: state.spotify.library_playlists,
		local_library_playlists: state.mopidy.library_playlists,
		mopidy_connected: state.mopidy.connected,
		spotify_authorized: state.spotify.authorization,
		spotify_userid: (state.spotify.me && state.spotify.me.id ? state.spotify.me.id : null)
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Playlist)
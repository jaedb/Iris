
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'
import FontAwesome from 'react-fontawesome'
import ReactGA from 'react-ga'

import TrackList from '../components/TrackList'
import Thumbnail from '../components/Thumbnail'
import Dater from '../components/Dater'
import ConfirmationButton from '../components/ConfirmationButton'
import LazyLoadListener from '../components/LazyLoadListener'
import FollowButton from '../components/FollowButton'
import Header from '../components/Header'
import ContextMenuTrigger from '../components/ContextMenuTrigger'

import * as helpers from '../helpers'
import * as coreActions from '../services/core/actions'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Playlist extends React.Component{

	constructor(props){
		super(props);
	}

	componentDidMount(){
		this.loadPlaylist();
	}

	componentWillReceiveProps(nextProps){
		if (nextProps.params.uri != this.props.params.uri){
			this.loadPlaylist(nextProps )
		}else if (!this.props.mopidy_connected && nextProps.mopidy_connected){
			if (helpers.uriSource(this.props.params.uri ) != 'spotify'){
				this.loadPlaylist(nextProps )
			}
		}
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

	loadPlaylist(props = this.props){

		if (props.playlist && props.playlist.tracks && (props.playlist.tracks_total == 0 || props.playlist.tracks.length > 0)){
			console.info('Loading playlist from index')

		} else {
			switch (helpers.uriSource(props.params.uri)){

				case 'spotify':
					this.props.spotifyActions.getPlaylist(props.params.uri )
					break

				default:
					if (props.mopidy_connected){
						this.props.mopidyActions.getPlaylist(props.params.uri )
					}
					break
			}
		}
	}

	loadMore(){
		this.props.spotifyActions.getURL(this.props.playlist.tracks_more, 'PLAYLIST_LOADED_MORE_TRACKS', this.props.playlist.uri );
	}

	play(){
        this.props.mopidyActions.playPlaylist(this.props.playlist.uri)
	}

	follow(){
        ReactGA.event({ category: 'Playlist', action: 'Follow', label: this.props.playlist.uri })
		this.props.spotifyActions.toggleFollowingPlaylist(this.props.playlist.uri, 'PUT')
	}

	// TODO: Once unfollowing occurs, remove playlist from global playlists list
	unfollow(){
        ReactGA.event({ category: 'Playlist', action: 'Unfollow', label: this.props.playlist.uri })
		this.props.spotifyActions.toggleFollowingPlaylist(this.props.playlist.uri, 'DELETE' )
	}

	// TODO: Once deletion occurs, remove playlist from global playlists list
	delete(){
		this.props.mopidyActions.deletePlaylist(this.props.playlist.uri )
	}

	reorderTracks(indexes, index){
		this.props.coreActions.reorderPlaylistTracks(this.props.playlist.uri, indexes, index, this.props.playlist.snapshot_id )
	}

	removeTracks(tracks_indexes){
		this.props.coreActions.removeTracksFromPlaylist(this.props.playlist.uri, tracks_indexes )
	}

	inLibrary(){
		var library = helpers.uriSource(this.props.params.uri)+'_library_playlists'
		return (this.props[library] && this.props[library].indexOf(this.props.params.uri) > -1)
	}

	renderActions(){
		switch(helpers.uriSource(this.props.playlist.uri )){

			case 'm3u':
				return (
					<div className="actions">
						<button className="primary" onClick={ e => this.play() }>Play</button>
						<button className="secondary" onClick={ e => this.props.uiActions.openModal('edit_playlist', { uri: this.props.params.uri, name: this.props.playlist.name }) }>Edit</button>
						<ContextMenuTrigger onTrigger={e => this.handleContextMenu(e)} />
					</div>
				)

			case 'spotify':
				if (this.props.playlist.can_edit){
					return (
						<div className="actions">
							<button className="primary" onClick={ e => this.play() }>Play</button>
							<button className="secondary" onClick={ e => this.props.uiActions.openModal('edit_playlist', { uri: this.props.params.uri, name: this.props.playlist.name, public: this.props.playlist.public, collaborative: this.props.playlist.collaborative, description: this.props.playlist.description }) }>Edit</button>
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
		if (!this.props.playlist) return null

		var scheme = helpers.uriSource(this.props.params.uri )
		var context = 'playlist'
		if (this.props.playlist.can_edit) context = 'editable-playlist'
		var user_id = helpers.getFromUri('userid',this.props.params.uri)
		var playlist_id = helpers.getFromUri('playlistid',this.props.params.uri)

		if (helpers.isLoading(this.props.load_queue,['spotify_users/'+user_id+'/playlists/'+playlist_id+'?'])){
			return (
				<div className="body-loader loading">
					<div className="loader"></div>
				</div>
			)
		}

		return (
			<div className="view playlist-view content-wrapper">
				<div className="thumbnail-wrapper">
					<Thumbnail size="large" canZoom images={ this.props.playlist.images } />
				</div>

				<div className="title">
					<h1>{ this.props.playlist.name }</h1>
					{ this.props.playlist.description ? <h2 className="description grey-text" dangerouslySetInnerHTML={{__html: this.props.playlist.description}}></h2> : null }

					<ul className="details">
						{ !this.props.slim_mode ? <li className="has-tooltip"><FontAwesome name={helpers.sourceIcon(this.props.params.uri )} /><span className="tooltip">{helpers.uriSource(this.props.params.uri )} playlist</span></li> : null }
						{ this.props.playlist.owner && !this.props.slim_mode ? <li><Link to={'/user/'+this.props.playlist.owner.uri}>{this.props.playlist.owner.id}</Link></li> : null }
						{ this.props.playlist.followers ? <li>{this.props.playlist.followers.total.toLocaleString()} followers</li> : null }
						{ this.props.playlist.last_modified ? <li><Dater type="ago" data={this.props.playlist.last_modified} /></li> : null }
						<li>
							{ this.props.playlist.tracks_total ? this.props.playlist.tracks_total : '0'} tracks,&nbsp;
							{ this.props.playlist.tracks ? <Dater type="total-time" data={this.props.playlist.tracks} /> : '0 mins' }
						</li>
					</ul>
				</div>

				{ this.renderActions() }

				<section className="list-wrapper">
					{ this.props.playlist.tracks ? <TrackList uri={this.props.params.uri} className="playlist-track-list" context={context} tracks={this.props.playlist.tracks} removeTracks={ tracks_indexes => this.removeTracks(tracks_indexes) } reorderTracks={ (indexes, index) => this.reorderTracks(indexes, index) } /> : null }
					<LazyLoadListener loading={this.props.playlist.tracks_more} loadMore={ () => this.loadMore() }/>
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
	var uri = ownProps.params.uri
	uri = uri.replace(' ','%20')
	return {
		slim_mode: state.ui.slim_mode,
		load_queue: state.ui.load_queue,
		playlist: (state.core.playlists && state.core.playlists[uri] !== undefined ? state.core.playlists[uri] : false ),
		spotify_library_playlists: state.spotify.library_playlists,
		local_library_playlists: state.mopidy.library_playlists,
		mopidy_connected: state.mopidy.connected,
		spotify_authorized: state.spotify.authorization,
		spotify_userid: state.spotify.me.id
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
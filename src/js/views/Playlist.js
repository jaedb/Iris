
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
import SidebarToggleButton from '../components/SidebarToggleButton'
import ContextMenuTrigger from '../components/ContextMenuTrigger'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Playlist extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.loadPlaylist();
	}

	componentWillReceiveProps( nextProps ){
		if( nextProps.params.uri != this.props.params.uri ){
			this.loadPlaylist( nextProps )
		}else if( !this.props.mopidy_connected && nextProps.mopidy_connected ){
			if( helpers.uriSource( this.props.params.uri ) != 'spotify' ){
				this.loadPlaylist( nextProps )
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

	loadPlaylist( props = this.props ){
		switch( helpers.uriSource( props.params.uri ) ){

			case 'spotify':
				this.props.spotifyActions.getPlaylist( props.params.uri );
				break

			default:
				if( props.mopidy_connected ) this.props.mopidyActions.getPlaylist( props.params.uri );
				break

		}
	}

	loadMore(){
		this.props.spotifyActions.getURL( this.props.playlist.tracks_more, 'PLAYLIST_LOADED_MORE_TRACKS', this.props.playlist.uri );
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
		this.props.spotifyActions.toggleFollowingPlaylist( this.props.playlist.uri, 'DELETE' )
	}

	// TODO: Once deletion occurs, remove playlist from global playlists list
	delete(){
		this.props.mopidyActions.deletePlaylist( this.props.playlist.uri )
	}

	reorderTracks( indexes, index ){
		this.props.uiActions.reorderPlaylistTracks( this.props.playlist.uri, indexes, index, this.props.playlist.snapshot_id )
	}

	removeTracks( tracks_indexes ){
		this.props.uiActions.removeTracksFromPlaylist( this.props.playlist.uri, tracks_indexes )
	}

	inLibrary(){
		return (this.props.library_playlists && this.props.library_playlists.indexOf(this.props.params.uri) > -1)
	}

	renderActions(){
		switch( helpers.uriSource( this.props.playlist.uri ) ){

			case 'm3u':
				return (
					<div className="actions">
						<button className="primary" onClick={ e => this.play() }>Play</button>
						<button className="secondary" onClick={ e => this.props.uiActions.openModal('edit_playlist', { uri: this.props.playlist.uri, name: this.props.playlist.name }) }>Edit</button>
						<ContextMenuTrigger onTrigger={e => this.handleContextMenu(e)} />
					</div>
				)

			case 'spotify':
				if( this.props.playlist.can_edit ){
					return (
						<div className="actions">
							<button className="primary" onClick={ e => this.play() }>Play</button>
							<button className="secondary" onClick={ e => this.props.uiActions.openModal('edit_playlist', { uri: this.props.playlist.uri, name: this.props.playlist.name, is_public: this.props.playlist.public }) }>Edit</button>
							<ContextMenuTrigger onTrigger={e => this.handleContextMenu(e)} />
						</div>
					)
				}
				return (
					<div className="actions">
						<button className="primary" onClick={ e => this.play() }>Play</button>
						<FollowButton className="secondary" uri={this.props.playlist.uri} addText="Add to library" removeText="Remove from library" is_following={this.inLibrary()} />
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

		var scheme = helpers.uriSource( this.props.params.uri )
		var context = 'playlist'
		var user_id = helpers.getFromUri('userid',this.props.params.uri)
		var playlist_id = helpers.getFromUri('playlistid',this.props.params.uri)

		if (helpers.isLoading(this.props.load_queue,['spotify_users/'+user_id+'/playlists/'+playlist_id+'?'])){
			return (
				<div className="body-loader">
					<div className="loader"></div>
				</div>
			)
		}

		if (this.props.playlist){
			if (this.props.playlist.can_edit) context = 'editable-playlist'

			return (
				<div className="view playlist-view">
				
					<SidebarToggleButton />

					<Thumbnail size="large" canZoom images={ this.props.playlist.images } />

					<div className="title">
						<div className="source grey-text">
							<FontAwesome name={helpers.sourceIcon( this.props.params.uri )} /> {helpers.uriSource( this.props.params.uri )} playlist
						</div>
						<h1>{ this.props.playlist.name }</h1>
						{ this.props.playlist.description ? <div className="description grey-text" dangerouslySetInnerHTML={{__html: this.props.playlist.description}}></div> : null }

						<ul className="details">
							{ this.props.playlist.owner ? <li><Link to={'/user/'+this.props.playlist.owner.uri}>{this.props.playlist.owner.id}</Link></li> : null }

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
						<LazyLoadListener enabled={this.props.playlist.tracks_more} loadMore={ () => this.loadMore() }/>
					</section>
				</div>
			)

		} else {
			return (
				<div className="view playlist-view">				
					<SidebarToggleButton />
					<Thumbnail size="large" />
					<div className="title">
						<div className="source grey-text">
							Playlist
						</div>
						<h1><span className="placeholder"></span></h1>
						<ul className="details">
							<li>
								<span className="placeholder"></span>
							</li>
						</ul>
					</div>					
					<div className="actions">
						<button className="placeholder">&nbsp;</button>
					</div>
				</div>
			)
		}
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
		load_queue: state.ui.load_queue,
		playlist: (state.ui.playlists && typeof(state.ui.playlists[uri]) !== 'undefined' ? state.ui.playlists[uri] : false ),
		library_playlists: state.ui.library_playlists,
		mopidy_connected: state.mopidy.connected,
		spotify_authorized: state.spotify.authorized,
		spotify_userid: state.spotify.me.id
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Playlist)

import React, { PropTypes } from 'react'
import { Link, hashHistory } from 'react-router'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import * as helpers from '../helpers'
import * as coreActions from '../services/core/actions'
import * as uiActions from '../services/ui/actions'
import * as pusherActions from '../services/pusher/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as lastfmActions from '../services/lastfm/actions'
import * as spotifyActions from '../services/spotify/actions'
import TrackList from './TrackList'

class ContextMenu extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			submenu_expanded: false
		}
		this.handleScroll = this.handleScroll.bind(this)
		this.handleMouseDown = this.handleMouseDown.bind(this)
	}

	componentDidMount(){
		window.addEventListener("scroll", this.handleScroll, false)
		window.addEventListener("mousedown", this.handleMouseDown, false)
	}

	componentWillUnmount(){		
		window.removeEventListener("scroll", this.handleScroll, false)
		window.removeEventListener("mousedown", this.handleMouseDown, false)
	}

	componentWillReceiveProps(nextProps){

		// if we've been given a menu object (ie activated) when we didn't have one prior
		if (nextProps.menu && !this.props.menu){			
			this.setState({ submenu_expanded: false })
			$('body').addClass('context-menu-open')

			var context = this.getContext(nextProps);

			// if we're able to be in the library, run a check
			if (nextProps.spotify_authorized && context.source == 'spotify'){
				switch (nextProps.menu.context){
					case 'artist':
					case 'album':
					case 'playlist':
						this.props.spotifyActions.following(nextProps.menu.items[0].uri)
						break
				}
			}

			// if we're able to be in the LastFM library, run a check
			if (nextProps.lastfm_authorized && context.is_track && context.items_count == 1){
				if (nextProps.menu.items[0].uri && this.props.tracks[nextProps.menu.items[0].uri] !== undefined && this.props.tracks[nextProps.menu.items[0].uri].userloved === undefined){
					this.props.lastfmActions.getTrack(nextProps.menu.items[0].uri);
				}
			}

		// we DID have one prior, and now we don't
		} else if (this.props.menu && !nextProps.menu){
			$('body').removeClass('context-menu-open')
		}
	}

	handleScroll(e){
		if (this.props.menu){
			this.props.uiActions.hideContextMenu()
		}
	}

	handleMouseDown(e){
		// if we click outside of the context menu or context menu trigger, kill it
		if ($(e.target).closest('.context-menu').length <= 0 && $(e.target).closest('.context-menu-trigger').length <= 0){
			this.props.uiActions.hideContextMenu()
		}
	}

	getContext(props = this.props){
		var context = {
			name: null,
			nice_name: 'Unknown',
			is_track: false
		}

		if (props.menu && props.menu.context){
			context.name = props.menu.context;
			context.nice_name = props.menu.context;

			// handle ugly labels
			switch (props.menu.context){
				case 'playlist':
				case 'editable-playlist':
					context.nice_name = 'playlist';
					break

				case 'track':
				case 'queue-track':
				case 'playlist-track':
				case 'editable-playlist-track':
					context.nice_name = 'track';
					context.is_track = true;
					break
			}

			// Consider the object(s) themselves
			// We can only really accommodate the first item. The only instances where
			// there is multiple is tracklists, when they're all of the same source (except search?)
			if (props.menu.items && props.menu.items.length > 0){
				var item = props.menu.items[0]
				context.item = item;
				context.items_count = props.menu.items.length;
				context.source = helpers.uriSource(item.uri);
				context.type = helpers.uriType(item.uri);
				context.in_library = this.inLibrary(item);
				context.is_loved = this.isLoved(item);
			}
		}

		return context
	}

	inLibrary(item = null){
		if (!item){
			return false
		}

		switch (helpers.uriType(item.uri)){
			case 'artist':
				return (this.props.spotify_library_artists && this.props.spotify_library_artists.indexOf(item.uri) > -1)
				break
			case 'album':
				return (this.props.spotify_library_albums && this.props.spotify_library_albums.indexOf(item.uri) > -1)
				break
			case 'playlist':
				return (this.props.spotify_library_playlists && this.props.spotify_library_playlists.indexOf(item.uri) > -1)
				break
		}
		return false
	}

	/**
	 * TODO: Currently the select track keys are the only details available. We need
	 * the actual track object reference (including name and artists) to getTrack from LastFM
	 **/
	isLoved(item = null){
		if (!item){
			return false
		}

		if (this.props.tracks[item.uri] === undefined){
			return false;
		}
		var track = this.props.tracks[item.uri];

		return (track.userloved !== undefined && track.userloved == "1");
	}

	canBeInLibrary(){
		if (!this.props.spotify_authorized){
			return false
		}
		return (helpers.uriSource(this.props.menu.items[0].uri) == 'spotify')
	}

	toggleInLibrary(e, in_library){
		this.props.uiActions.hideContextMenu()
		if (in_library){
			this.props.spotifyActions.following(this.props.menu.items[0].uri, 'DELETE')
		} else {
			this.props.spotifyActions.following(this.props.menu.items[0].uri, 'PUT')
		}
	}

	playQueueItem(e){
		this.props.uiActions.hideContextMenu()
		var tracks = this.props.menu.items;
		this.props.mopidyActions.changeTrack(tracks[0].tlid )
	}

	removeFromQueue(e){
		this.props.uiActions.hideContextMenu()
		var tracks = this.props.menu.items;
		var tracks_tlids = [];
		for(var i = 0; i < tracks.length; i++){
			tracks_tlids.push(tracks[i].tlid );
		}
		this.props.mopidyActions.removeTracks(tracks_tlids )
	}

	playURIs(e){
		this.props.uiActions.hideContextMenu()
		this.props.mopidyActions.playURIs(this.props.menu.uris, this.props.menu.tracklist_uri)
	}

	playPlaylist(e){
		this.props.uiActions.hideContextMenu()
		this.props.mopidyActions.playPlaylist(this.props.menu.uris[0])
	}

	playArtistTopTracks(e){
		this.props.uiActions.hideContextMenu()
		this.props.spotifyActions.playArtistTopTracks(this.props.menu.uris[0])
	}

	addToQueue(e, next = false){
		this.props.uiActions.hideContextMenu()
		this.props.mopidyActions.enqueueURIs(this.props.menu.uris, this.props.menu.tracklist_uri, next)
	}

	addTracksToPlaylist(e, playlist_uri){
		this.props.uiActions.hideContextMenu()
		this.props.coreActions.addTracksToPlaylist(playlist_uri, this.props.menu.uris)
	}

	toggleLoved(e, is_loved){
		this.props.uiActions.hideContextMenu()
		if (is_loved){
			this.props.lastfmActions.unloveTrack(this.props.menu.items[0].uri)
		} else {
			this.props.lastfmActions.loveTrack(this.props.menu.items[0].uri)
		}
	}

	unloveTrack(e){
		this.props.uiActions.hideContextMenu()
		this.props.lastfmActions.unloveTrack(this.props.menu.items[0])
	}

	removeFromPlaylist(e){
		this.props.uiActions.hideContextMenu()
		this.props.coreActions.removeTracksFromPlaylist(this.props.menu.tracklist_uri, this.props.menu.indexes)
	}

	deletePlaylist(e){
		this.props.uiActions.hideContextMenu()
		this.props.coreActions.deletePlaylist(this.props.menu.uris[0])
	}

	startRadio(e){
		this.props.uiActions.hideContextMenu()
		this.props.pusherActions.startRadio(this.props.menu.uris)
	}

	goToRecommendations(e){
		this.props.uiActions.hideContextMenu()
		var uris_string = helpers.arrayOf('uri',this.props.menu.items).join(',')
		hashHistory.push(global.baseURL +'discover/recommendations/'+ uris_string )
	}

	goToArtist(e){
		if (!this.props.menu.items || this.props.menu.items.length <= 0 || !this.props.menu.items[0].artists || this.props.menu.items[0].artists.length <= 0){
			return null
		} else {
			this.props.uiActions.hideContextMenu()
			hashHistory.push(global.baseURL +'artist/'+ this.props.menu.items[0].artists[0].uri )
		}
	}

	goToUser(e){
		if (!this.props.menu.items || this.props.menu.items.length <= 0){
			return null
		} else {
			this.props.uiActions.hideContextMenu()
			hashHistory.push(global.baseURL +'user/'+ this.props.menu.items[0].owner.uri )
		}
	}

	goToTrack(e){
		if (!this.props.menu.items || this.props.menu.items.length <= 0){
			return null
		} else {
			this.props.uiActions.hideContextMenu()
			hashHistory.push(global.baseURL +'track/'+ encodeURIComponent(this.props.menu.items[0].uri))
		}
	}

	copyURIs(e){
		var temp = $("<input>");
		$("body").append(temp);
		temp.val(this.props.menu.uris.join(',')).select();
		document.execCommand("copy");
		temp.remove();

		this.props.uiActions.createNotification("Copied "+this.props.menu.uris.length+" URIs" )
		this.props.uiActions.hideContextMenu()
	}

	closeAndDeselectTracks(e){
		this.props.uiActions.hideContextMenu();
		// TODO
	}

	renderPlaylistSubmenu(){
		var playlists = []
		for (var uri in this.props.playlists){
			if (this.props.playlists[uri].can_edit) playlists.push(this.props.playlists[uri])
		}

		playlists = helpers.sortItems(playlists, 'name')

		var loader = null
		if (this.props.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR && this.props.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR.status == 'running'){
			loader = (
				<div className="menu-item-wrapper">
					<div className="menu-item mini-loader loading">
						<div className="loader"></div>
					</div>
				</div>
			)
		}

		var list = <span className="menu-item-wrapper"><span className="menu-item grey-text">No writable playlists</span></span>
		if (playlists.length > 0){
			list = playlists.map(playlist => {
				return (
					<span className="menu-item-wrapper" key={playlist.uri}>
						<a className="menu-item" onClick={e => this.addTracksToPlaylist(e,playlist.uri) }>
							<span className="label">{ playlist.name }</span>
						</a>
					</span>
				)
			})
		}

		return (			
			<div className={this.state.submenu_expanded ? 'submenu expanded' : 'submenu'}>
				<span className="menu-item-wrapper">
					<a className="menu-item close-submenu" onClick={e => this.setState({submenu_expanded: false})}>
						<span className="label">
							<FontAwesome name='caret-left' />
							&nbsp;&nbsp;
							Back
						</span>
					</a>
				</span>
				{list}
				{loader}
			</div>
		)
	}

	renderTitle(){
		var context = this.getContext()

		if (context.name == 'custom'){
			return (
				<span className="title">
					<div className="background generic"></div>
					<div className="text">
						{this.props.menu.title}
					</div>
				</span>
			)
		}

		switch (context.type){

			case 'artist':
			case 'album':
			case 'playlist':
				var style = null
				if (context.item && context.item.images){
					style = {
						backgroundImage: 'url('+helpers.sizedImages(context.item.images).medium+')'
					}
				}

				return (
					<Link className="title" to={global.baseURL+context.type+'/'+context.item.uri}>
						{style ? <div className="background" style={style}></div> : null}
						<div className="type">
							{context.source}
							&nbsp;
							{context.nice_name}
						</div>
						<div className="text">{context.item.name}</div>
					</Link>
				)
				break

			default:
				return (
					<span className="title">
						<div className="type">
							{context.source}
							&nbsp;
							{context.nice_name}s
						</div>
						<div className="text">							
							{context.items_count} items
						</div>
						<div className="background generic"></div>
					</span>
				)
				break

		}
	}

	setPlaylistSubmenu(expanded = !this.state.submenu_expanded){
		this.setState({submenu_expanded: expanded})

		if (expanded){
			if (!this.props.spotify_library_playlists_loaded_all){
				this.props.spotifyActions.getLibraryPlaylists()
			}
			if (!this.props.mopidy_library_playlists_loaded_all){
				this.props.mopidyActions.getLibraryPlaylists()
			}
		}
	}

	renderItems(){
		var context = this.getContext()

		var play_uris = (
			<span className="menu-item-wrapper">
				<a className="menu-item" onClick={e => this.playURIs(e)}>
					<span className="label">Play</span>
				</a>
			</span>
		)

		var play_playlist = (
			<span className="menu-item-wrapper">
				<a className="menu-item" onClick={e => this.playPlaylist(e)}>
					<span className="label">Play</span>
				</a>
			</span>
		)

		var play_queue_item = (
			<span className="menu-item-wrapper">
				<a className="menu-item" onClick={e => this.playQueueItem(e)}>
					<span className="label">Play</span>
				</a>
			</span>
		)

		var play_uris_next = (
			<span className="menu-item-wrapper">
				<a className="menu-item" onClick={e => this.addToQueue(e, true)}>
					<span className="label">Play next</span>
				</a>
			</span>
		)

		var play_artist_top_tracks = (
			<span className="menu-item-wrapper">
				<a className="menu-item" onClick={e => this.playArtistTopTracks(e)}>
					<span className="label">Play top tracks</span>
				</a>
			</span>
		)

		var add_to_queue = (
			<span className="menu-item-wrapper">
				<a className="menu-item" onClick={e => this.addToQueue(e)}>
					<span className="label">Add to queue</span>
				</a>
			</span>
		)

		var add_to_playlist = (
			<span className="menu-item-wrapper has-submenu">
				<a className="menu-item" onClick={e => this.setPlaylistSubmenu()}>
					<span className="label">Add to playlist</span>
					<FontAwesome className="submenu-icon" name='caret-right' />
				</a>
				{this.renderPlaylistSubmenu()}
			</span>
		)

		var toggle_in_library = (
			<span className="menu-item-wrapper">
				<a className="menu-item" onClick={e => this.toggleInLibrary(e, context.in_library)}>
					<span className="label">
						{context.in_library ? 'Remove from library' : 'Add to library'}
					</span>
				</a>
			</span>
		)

		if (!this.props.lastfm_authorized){
			var toggle_loved = null;
		} else if (helpers.isLoading(this.props.load_queue,['lastfm_track.getInfo'])){
			var toggle_loved = (
				<span className="menu-item-wrapper">
					<a className="menu-item">
						<span className="label grey-text">
							Love track
						</span>
					</a>
				</span>
			)
		} else {			
			var toggle_loved = (
				<span className="menu-item-wrapper">
					<a className="menu-item" onClick={e => this.toggleLoved(e, context.is_loved)}>
						<span className="label">
							{context.is_loved ? 'Unlove' : 'Love'} track
						</span>
					</a>
				</span>
			)
		}

		var go_to_artist = (
			<span className="menu-item-wrapper">
				<a className="menu-item" onClick={e => this.goToArtist(e)}>
					<span className="label">Go to artist</span>
				</a>
			</span>
		)

		var go_to_user = (
			<span className="menu-item-wrapper">
				<a className="menu-item" onClick={e => this.goToUser(e)}>
					<span className="label">Go to user</span>
				</a>
			</span>
		)

		var go_to_track = (
			<span className="menu-item-wrapper">
				<a className="menu-item" onClick={e => this.goToTrack(e)}>
					<span className="label">Track info</span>
				</a>
			</span>
		)

		var go_to_recommendations = (
			<span className="menu-item-wrapper">
				<a className="menu-item" onClick={e => this.goToRecommendations(e)}>
					<span className="label">Discover similar</span>
				</a>
			</span>
		)

		var start_radio = (
			<span className="menu-item-wrapper">
				<a className="menu-item" onClick={e => this.startRadio(e)}>
					<span className="label">Start radio</span>
				</a>
			</span>
		)

		var remove_from_queue = (
			<span className="menu-item-wrapper">
				<a className="menu-item" onClick={e => this.removeFromQueue(e)}>
					<span className="label">Remove</span>
				</a>
			</span>
		)

		var remove_from_playlist = (
			<span className="menu-item-wrapper">
				<a className="menu-item" onClick={e => this.removeFromPlaylist(e)}>
					<span className="label">Remove</span>
				</a>
			</span>
		)

		var delete_playlist = (
			<span className="menu-item-wrapper">
				<a className="menu-item" onClick={e => this.deletePlaylist(e)}>
					<span className="label">Delete</span>
				</a>
			</span>
		)

		var copy_uris = (
			<span className="menu-item-wrapper">
				<a className="menu-item" onClick={e => this.copyURIs(e)}>
					<span className="label">Copy URI{context.items_count > 1 ? 's' : ''}</span>
				</a>
			</span>
		)

		switch (context.name){

			case 'album':
				return (
					<div>
						{play_uris}
						{play_uris_next}
						{add_to_queue}
						<div className="divider" />
						{go_to_artist}
						{copy_uris}
						{this.canBeInLibrary() ? toggle_in_library : null}
					</div>
				)
				break

			case 'artist':
				return (
					<div>
						{context.source == 'spotify' ? play_artist_top_tracks : null}
						{context.source == 'spotify' ? start_radio : null}
						<div className="divider" />
						{context.source == 'spotify' ? go_to_recommendations : null}
						{copy_uris}
						{this.canBeInLibrary() ? <div className="divider" /> : null}
						{this.canBeInLibrary() ? toggle_in_library : null}
					</div>
				)
				break

			case 'playlist':
				return (
					<div>
						{play_playlist}
						<div className="divider" />
						{context.source == 'spotify' ? go_to_user : null}
						{copy_uris}
						{this.canBeInLibrary() ? <div className="divider" /> : null}
						{this.canBeInLibrary() ? toggle_in_library : null}
					</div>
				)
				break

			case 'editable-playlist':
				return (
					<div>
						{play_playlist}
						<div className="divider" />
						{context.source == 'spotify' ? go_to_user : null}
						{copy_uris}
						<div className="divider" />
						{this.canBeInLibrary() ? toggle_in_library : null}
						{delete_playlist}
					</div>
				)
				break

			case 'queue-track':
				return (
					<div>
						{context.items_count == 1 ? play_queue_item : null}
						{context.items_count == 1 ? <div className="divider" /> : null}
						{add_to_playlist}
						{context.items_count == 1 ? toggle_loved : null}
						{context.source == 'spotify' && context.items_count <= 5 ? go_to_recommendations : null}
						{context.items_count == 1 ? go_to_track : null}
						<div className="divider" />
						{copy_uris}
						{remove_from_queue}
					</div>
				)
				break

			case 'editable-playlist-track':
				return (
					<div>
						{play_uris}
						{play_uris_next}
						{add_to_queue}
						{context.source == 'spotify' && context.items_count == 1 ? start_radio : null}
						<div className="divider" />
						{add_to_playlist}
						{context.items_count == 1 ? toggle_loved : null}
						{context.source == 'spotify' && context.items_count <= 5 ? go_to_recommendations : null}
						{context.items_count == 1 ? go_to_track : null}
						<div className="divider" />
						{copy_uris}
						{remove_from_playlist}
					</div>
				)
				break

			default:
				return (
					<div>
						{play_uris}
						{play_uris_next}
						{add_to_queue}
						{context.source == 'spotify' && context.items_count == 1 ? start_radio : null}
						<div className="divider" />
						{add_to_playlist}
						{context.items_count == 1 ? toggle_loved : null}
						{context.source == 'spotify' && context.items_count <= 5 ? go_to_recommendations : null}
						{context.items_count == 1 ? go_to_track : null}
						<div className="divider" />
						{copy_uris}
					</div>
				)
				break
		}
	}

	render(){
		if (!this.props.menu) return null;

		var style = {
			left: this.props.menu.position_x,
			top: this.props.menu.position_y,
		}
		var height = 200 // TODO: use jquery to detect height
		var className = "context-menu "+this.props.menu.context
		if (this.state.submenu_expanded){
			className += ' submenu-expanded'
		}

		if (this.props.menu.position_x > (window.innerWidth - 174)){
			style.left = 'auto'
			style.right = 10
		}
		if (this.props.menu.position_y > (window.innerHeight - height)){
			style.top = 'auto'
			style.bottom = 10
		}

		return (
			<div id="context-menu" className={className} style={style}>
				<div className="liner">
					{this.renderTitle()}
					{this.props.menu.context == 'custom' ? this.props.menu.options : this.renderItems()}
				</div>
				<div className="background" onClick={e => this.props.uiActions.hideContextMenu()}></div>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		menu: state.ui.context_menu,
		load_queue: state.ui.load_queue,
		processes: state.ui.processes,
		current_track: state.core.current_track,
		current_tracklist: state.core.current_tracklist,
		spotify_library_playlists: state.spotify.library_playlists,
		spotify_library_playlists_loaded_all: state.spotify.library_playlists_loaded_all,
		mopidy_library_playlists: state.mopidy.library_playlists,
		mopidy_library_playlists_loaded_all: state.mopidy.library_playlists_loaded_all,
		spotify_library_artists: state.spotify.library_artists,
		mopidy_library_artists: state.mopidy.library_artists,
		spotify_library_albums: state.spotify.library_albums,
		mopidy_library_albums: state.mopidy.library_albums,
		playlists: state.core.playlists,
		tracks: state.core.tracks,
		spotify_authorized: state.spotify.authorization,
		lastfm_authorized: state.lastfm.session
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		lastfmActions: bindActionCreators(lastfmActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(ContextMenu)
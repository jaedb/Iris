
import React, { PropTypes } from 'react'
import { hashHistory } from 'react-router'
import Link from './Link'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import TrackList from './TrackList'
import Icon from './Icon'
import Thumbnail from './Thumbnail'

import * as helpers from '../helpers'
import * as coreActions from '../services/core/actions'
import * as uiActions from '../services/ui/actions'
import * as pusherActions from '../services/pusher/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as lastfmActions from '../services/lastfm/actions'
import * as spotifyActions from '../services/spotify/actions'

class ContextMenu extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			submenu: null
		}
		this.handleScroll = this.handleScroll.bind(this);
		this.handleMouseDown = this.handleMouseDown.bind(this);
		this.handleTouchStart = this.handleTouchStart.bind(this);
	}

	componentDidMount(){
		window.addEventListener("scroll", this.handleScroll, false);
		window.addEventListener("mousedown", this.handleMouseDown, false);
		window.addEventListener("touchstart", this.handleTouchStart, false);
	}

	componentWillUnmount(){		
		window.removeEventListener("scroll", this.handleScroll, false);
		window.removeEventListener("mousedown", this.handleMouseDown, false);
		window.removeEventListener("touchstart", this.handleTouchStart, false);
	}

	componentWillReceiveProps(nextProps){

		// if we've been given a menu object (ie activated) when we didn't have one prior
		if (nextProps.menu && !this.props.menu){			
			this.setState({ submenu: null });

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
		}
	}

	handleScroll(e){
		if (this.props.menu){
			this.props.uiActions.hideContextMenu();
		}
	}

	handleMouseDown(e){
		// if we click (touch or mouse) outside of the context menu or context menu trigger, kill it
		if ($(e.target).closest('.context-menu').length <= 0 && $(e.target).closest('.context-menu-trigger').length <= 0){
			this.props.uiActions.hideContextMenu();
		}
	}

	handleTouchStart(e){

		// if we click (touch or mouse) outside of the context menu or context menu trigger, kill it
		if ($(e.target).closest('.context-menu').length <= 0 && $(e.target).closest('.context-menu-trigger').length <= 0){
			this.props.uiActions.hideContextMenu();
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
					break;

				case 'track':
				case 'queue-track':
				case 'playlist-track':
				case 'editable-playlist-track':
					context.nice_name = 'track';
					context.is_track = true;
					break;
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
			return false;
		}

		switch (helpers.uriType(item.uri)){
			case 'artist':
				return (this.props.spotify_library_artists && this.props.spotify_library_artists.indexOf(item.uri) > -1);
				break;
			case 'album':
				return (this.props.spotify_library_albums && this.props.spotify_library_albums.indexOf(item.uri) > -1);
				break;
			case 'playlist':
				return (this.props.spotify_library_playlists && this.props.spotify_library_playlists.indexOf(item.uri) > -1);
				break;
		}
		return false;
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
		this.props.mopidyActions.changeTrack(tracks[0].tlid);
	}

	removeFromQueue(e){
		this.props.uiActions.hideContextMenu()
		var tracks = this.props.menu.items;
		var tracks_tlids = [];
		for(var i = 0; i < tracks.length; i++){
			tracks_tlids.push(tracks[i].tlid );
		}
		this.props.mopidyActions.removeTracks(tracks_tlids);
	}

	playURIs(e){
		this.props.uiActions.hideContextMenu();
		this.props.mopidyActions.playURIs(this.props.menu.uris, this.props.menu.tracklist_uri);
	}

	playPlaylist(e){
		this.props.uiActions.hideContextMenu();
		this.props.mopidyActions.playPlaylist(this.props.menu.uris[0]);
	}

	shufflePlayPlaylist(e){
		this.props.uiActions.hideContextMenu();
		this.props.mopidyActions.playPlaylist(this.props.menu.uris[0], true);
	}

	playArtistTopTracks(e){
		this.props.uiActions.hideContextMenu();
		this.props.spotifyActions.playArtistTopTracks(this.props.menu.uris[0]);
	}

	addToQueue(e, next = false){
		this.props.uiActions.hideContextMenu();
		this.props.mopidyActions.enqueueURIs(this.props.menu.uris, this.props.menu.tracklist_uri, next);
	}

	addTracksToPlaylist(e, playlist_uri){
		this.props.uiActions.hideContextMenu();
		this.props.coreActions.addTracksToPlaylist(playlist_uri, this.props.menu.uris);
	}

	toggleLoved(e, is_loved){
		this.props.uiActions.hideContextMenu();
		if (is_loved){
			this.props.lastfmActions.unloveTrack(this.props.menu.items[0].uri);
		} else {
			this.props.lastfmActions.loveTrack(this.props.menu.items[0].uri);
		}
	}

	unloveTrack(e){
		this.props.uiActions.hideContextMenu();
		this.props.lastfmActions.unloveTrack(this.props.menu.items[0]);
	}

	removeFromPlaylist(e){
		this.props.uiActions.hideContextMenu();
		this.props.coreActions.removeTracksFromPlaylist(this.props.menu.tracklist_uri, this.props.menu.indexes);
	}

	deletePlaylist(e){
		this.props.uiActions.hideContextMenu();
		this.props.coreActions.deletePlaylist(this.props.menu.uris[0]);
	}

	startRadio(e){
		this.props.uiActions.hideContextMenu();
		this.props.pusherActions.startRadio(this.props.menu.uris);
	}

	goToRecommendations(e){
		this.props.uiActions.hideContextMenu();
		var uris_string = helpers.arrayOf('uri',this.props.menu.items).join(',');
		hashHistory.push(global.baseURL +'discover/recommendations/'+ uris_string );
	}

	goToArtist(e){
		if (!this.props.menu.items || this.props.menu.items.length <= 0 || !this.props.menu.items[0].artists_uris || this.props.menu.items[0].artists_uris.length <= 0){
			return null;
		} else {
			this.props.uiActions.hideContextMenu();
			
			// note: we can only go to one artist (even if this item has multiple artists, just go to the first one)
			hashHistory.push(global.baseURL +'artist/'+ this.props.menu.items[0].artists_uris[0]);
		}
	}

	goToAlbum(e){
		if (!this.props.menu.items || this.props.menu.items.length <= 0 || !this.props.menu.items[0].album_uri){
			return null;
		} else {
			this.props.uiActions.hideContextMenu();
			hashHistory.push(global.baseURL +'album/'+ this.props.menu.items[0].album_uri);
		}
	}

	goToUser(e){
		if (!this.props.menu.items || this.props.menu.items.length <= 0 || !this.props.menu.items[0].user_uri){
			return null;
		} else {
			this.props.uiActions.hideContextMenu();
			hashHistory.push(global.baseURL +'user/'+ this.props.menu.items[0].user_uri);
		}
	}

	goToTrack(e){
		if (!this.props.menu.items || this.props.menu.items.length <= 0){
			return null;
		} else {
			this.props.uiActions.hideContextMenu();
			hashHistory.push(global.baseURL +'track/'+ encodeURIComponent(this.props.menu.items[0].uri));
		}
	}

	copyURIs(e){
		var temp = $("<input>");
		$("body").append(temp);
		temp.val(this.props.menu.uris.join(',')).select();
		document.execCommand("copy");
		temp.remove();

		this.props.uiActions.createNotification({content: "Copied "+this.props.menu.uris.length+" URIs"});
		this.props.uiActions.hideContextMenu()
	}

	closeAndDeselectTracks(e){
		this.props.uiActions.hideContextMenu();
		// TODO
	}

	renderTitle(){

		var context = this.getContext()

		if (context.items_count > 1){
			return (
				<div className="context-menu__title">
					<div className="context-menu__title__text">							
						{context.items_count} {context.nice_name}{context.items_count > 1 ? 's' : null} selected
					</div>
				</div>
			)
		}

		if (context.name == 'custom'){
			if (!this.props.menu.title){
				return null;
			}

			return (
				<div className="context-menu__title">
					<div className="context-menu__title__text">
						{this.props.menu.title}
					</div>
				</div>
			)
		}
		
		// Do we need titles or does it just confuse things and rip off Spotify too hard?
		return null;
/*
		switch (context.type){

			case 'artist':
			case 'album':
			case 'playlist':
				var style = null;

				return (
					<div className="context-menu__title">
						<Thumbnail size="small" images={context.item ? context.item.images : null} circle={context.type == 'artist'} />
						<div className="context-menu__title__text">{context.item.name}</div>
						<div className="context-menu__title__type">
							{context.source}
						</div>
					</div>
				);

			default:
				return (
					<div className="context-menu__title">
						<div className="context-menu__title__text">							
							{context.items_count} {context.nice_name}{context.items_count > 1 ? 's' : null}
						</div>
						<div className="context-menu__title__type">
							{context.source}
						</div>
					</div>
				);

		}*/
	}

	setSubmenu(name){

		if (this.state.submenu !== name && name == 'add-to-playlist'){
			if (!this.props.spotify_library_playlists_loaded_all){
				this.props.spotifyActions.getLibraryPlaylists()
			}
			if (!this.props.mopidy_library_playlists_loaded_all){
				this.props.mopidyActions.getLibraryPlaylists()
			}
		}

		this.setState({submenu: name});
	}

	renderSubmenu(){
		var list = null;
		var loader = null;

		switch (this.state.submenu){

			case 'add-to-playlist':

				var playlists = []
				for (var uri in this.props.playlists){
					if (this.props.playlists[uri].can_edit) playlists.push(this.props.playlists[uri])
				}

				playlists = helpers.sortItems(playlists, 'name')

				if (this.props.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR && this.props.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR.status == 'running'){
					loader = (
						<div className="context-menu__item">
							<div className="context-menu__item mini-loader loading">
								<div className="loader"></div>
							</div>
						</div>
					)
				}

				list = <span className="context-menu__item"><span className="context-menu__item mid_grey-text">No writable playlists</span></span>
				if (playlists.length > 0){
					list = playlists.map(playlist => {
						return (
							<span className="context-menu__item" key={playlist.uri}>
								<a className="context-menu__item__link" onClick={e => this.addTracksToPlaylist(e,playlist.uri) }>
									<span className="context-menu__item__label">{ playlist.name }</span>
								</a>
							</span>
						)
					})
				}

		}

		return (
			<div className='context-menu__section context-menu__section--submenu'>
				<div className="context-menu__item">
					<a className="context-menu__item__link context-menu__item__link--close-submenu" onClick={e => this.setState({submenu: null})}>
						<span className="context-menu__item__label">
							<Icon name="arrow_back" /> Back
						</span>
					</a>
				</div>
				{list}
				{loader}
			</div>
		);
	}

	renderItems(){
		var context = this.getContext()

		var play_uris = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.playURIs(e)}>
					<span className="context-menu__item__label">Play</span>
				</a>
			</div>
		)

		var play_playlist = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.playPlaylist(e)}>
					<span className="context-menu__item__label">Play</span>
				</a>
			</div>
		)

		var shuffle_play_playlist = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.shufflePlayPlaylist(e)}>
					<span className="context-menu__item__label">Shuffle play</span>
				</a>
			</div>
		)

		var play_queue_item = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.playQueueItem(e)}>
					<span className="context-menu__item__label">Play</span>
				</a>
			</div>
		)

		var play_uris_next = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.addToQueue(e, true)}>
					<span className="context-menu__item__label">Play next</span>
				</a>
			</div>
		)

		var play_artist_top_tracks = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.playArtistTopTracks(e)}>
					<span className="context-menu__item__label">Play top tracks</span>
				</a>
			</div>
		)

		var add_to_queue = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.addToQueue(e)}>
					<span className="context-menu__item__label">Add to queue</span>
				</a>
			</div>
		)

		var add_to_playlist = (
			<div className="context-menu__item context-menu__item--has-submenu">
				<a className="context-menu__item__link" onClick={e => this.setSubmenu('add-to-playlist')}>
					<span className="context-menu__item__label">Add to playlist</span>
					<Icon className="submenu-icon" name="arrow_forward" />
				</a>
			</div>
		)

		var toggle_in_library = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.toggleInLibrary(e, context.in_library)}>
					<span className="context-menu__item__label">
						{context.in_library ? 'Remove from library' : 'Add to library'}
					</span>
				</a>
			</div>
		)

		if (!this.props.lastfm_authorized){
			var toggle_loved = null;
		} else if (helpers.isLoading(this.props.load_queue,['lastfm_track.getInfo'])){
			var toggle_loved = (
				<div className="context-menu__item">
					<a className="context-menu__item__link">
						<span className="context-menu__item__label mid_grey-text">
							Love track
						</span>
					</a>
				</div>
			)
		} else {			
			var toggle_loved = (
				<div className="context-menu__item">
					<a className="context-menu__item__link" onClick={e => this.toggleLoved(e, context.is_loved)}>
						<span className="context-menu__item__label">
							{context.is_loved ? 'Unlove' : 'Love'} track
						</span>
					</a>
				</div>
			)
		}

		var go_to_artist = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.goToArtist(e)}>
					<span className="context-menu__item__label">Go to artist</span>
				</a>
			</div>
		)

		var go_to_album = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.goToAlbum(e)}>
					<span className="context-menu__item__label">Go to album</span>
				</a>
			</div>
		)

		var go_to_user = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.goToUser(e)}>
					<span className="context-menu__item__label">Go to user</span>
				</a>
			</div>
		)

		var go_to_track = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.goToTrack(e)}>
					<span className="context-menu__item__label">Track info</span>
				</a>
			</div>
		)

		var go_to_recommendations = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.goToRecommendations(e)}>
					<span className="context-menu__item__label">Discover similar</span>
				</a>
			</div>
		)

		var start_radio = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.startRadio(e)}>
					<span className="context-menu__item__label">Start radio</span>
				</a>
			</div>
		)

		var remove_from_queue = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.removeFromQueue(e)}>
					<span className="context-menu__item__label">Remove</span>
				</a>
			</div>
		)

		var remove_from_playlist = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.removeFromPlaylist(e)}>
					<span className="context-menu__item__label">Remove</span>
				</a>
			</div>
		)

		var delete_playlist = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.deletePlaylist(e)}>
					<span className="context-menu__item__label">Delete</span>
				</a>
			</div>
		)

		var copy_uris = (
			<div className="context-menu__item">
				<a className="context-menu__item__link" onClick={e => this.copyURIs(e)}>
					<span className="context-menu__item__label">Copy URI{context.items_count > 1 ? 's' : ''}</span>
				</a>
			</div>
		)

		switch (context.name){

			case 'album':
				return (
					<div>
						{play_uris}
						{play_uris_next}
						{add_to_queue}
						{this.canBeInLibrary() ? <div className="context-menu__divider" /> : null}
						{this.canBeInLibrary() ? toggle_in_library : null}
						<div className="context-menu__divider" />
						{go_to_artist}
						{copy_uris}
					</div>
				)
				break

			case 'artist':
				return (
					<div>
						{context.source == 'spotify' ? play_artist_top_tracks : null}
						{context.source == 'spotify' ? start_radio : null}
						{this.canBeInLibrary() ? <div className="context-menu__divider" /> : null}
						{this.canBeInLibrary() ? toggle_in_library : null}
						<div className="context-menu__divider" />
						{context.source == 'spotify' ? go_to_recommendations : null}
						{copy_uris}
					</div>
				)
				break

			case 'playlist':
				return (
					<div>
						{play_playlist}
						{shuffle_play_playlist}
						{this.canBeInLibrary() ? <div className="context-menu__divider" /> : null}
						{this.canBeInLibrary() ? toggle_in_library : null}
						<div className="context-menu__divider" />
						{context.source == 'spotify' ? go_to_user : null}
						{copy_uris}
					</div>
				)
				break

			case 'editable-playlist':
				return (
					<div>
						{play_playlist}
						{shuffle_play_playlist}
						{this.canBeInLibrary() ? <div className="context-menu__divider" /> : null}
						{this.canBeInLibrary() ? toggle_in_library : null}
						<div className="context-menu__divider" />
						{context.source == 'spotify' ? go_to_user : null}
						{copy_uris}
						<div className="context-menu__divider" />
						{delete_playlist}
					</div>
				)
				break

			case 'queue-track':
				return (
					<div>
						{context.items_count == 1 ? play_queue_item : null}
						<div className="context-menu__divider" />
						{add_to_playlist}
						{toggle_loved}
						<div className="context-menu__divider" />
						{context.source == 'spotify' && context.items_count <= 5 ? go_to_recommendations : null}
						{context.items_count == 1 ? go_to_track : null}
						{copy_uris}
						<div className="context-menu__divider" />
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
						<div className="context-menu__divider" />
						{add_to_playlist}
						{toggle_loved}
						<div className="context-menu__divider" />
						{context.source == 'spotify' && context.items_count <= 5 ? go_to_recommendations : null}
						{context.items_count == 1 ? go_to_track : null}
						{copy_uris}
						<div className="context-menu__divider" />
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
						<div className="context-menu__divider" />
						{add_to_playlist}
						{toggle_loved}
						<div className="context-menu__divider" />
						{context.source == 'spotify' && context.items_count <= 5 ? go_to_recommendations : null}
						{context.items_count == 1 ? go_to_album : null}
						{context.items_count == 1 ? go_to_track : null}
						<div className="context-menu__divider" />
						{copy_uris}
					</div>
				)
				break
		}
	}

	render(){
		if (!this.props.menu){
			return null;
		}

		var style = {
			left: this.props.menu.position_x,
			top: this.props.menu.position_y,
		}
		var height = 200 // TODO: use jquery to detect height
		var className = "context-menu "+this.props.menu.context
		if (this.state.submenu){
			className += ' context-menu--submenu-expanded';
		}
		
		if (this.props.menu.closing){
			className += ' context-menu--closing';
		}

		if (this.props.menu.position_x > (window.innerWidth - 174)){
			style.left = 'auto';
			style.right = 10;
		}

		if (this.props.menu.position_y > (window.innerHeight - height)){
			style.top = 'auto';
			style.bottom = 10;
		}

		return (
			<div id="context-menu" className={className} style={style}>
				<div className="context-menu__section context-menu__section--items">
					{this.renderTitle()}
					{this.props.menu.context == 'custom' ? this.props.menu.options : this.renderItems()}
				</div>
				{this.renderSubmenu()}
				<div className="context-menu__background" onClick={e => this.props.uiActions.hideContextMenu()}></div>
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
		lastfm_authorized: state.lastfm.authorization
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
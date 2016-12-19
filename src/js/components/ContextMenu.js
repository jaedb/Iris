
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import TrackList from './TrackList'
import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class ContextMenu extends React.Component{

	constructor(props) {
		super(props);
		this.handleScroll = this.handleScroll.bind(this)
		this.handleClick = this.handleClick.bind(this)
	}

	componentDidMount(){		
		window.addEventListener("scroll", this.handleScroll, false)
		window.addEventListener("click", this.handleClick, false)
	}

	componentWillUnmount(){		
		window.removeEventListener("scroll", this.handleScroll, false)
		window.removeEventListener("click", this.handleClick, false)
	}

	handleScroll(){
		if( this.props.context_menu.show && this.props.context_menu.trigger == 'click' ){
			this.props.uiActions.hideContextMenu();
		}
	}

	handleClick(){
		if( this.props.context_menu.show && this.props.context_menu.trigger == 'click' ){
			this.props.uiActions.hideContextMenu();
		}
	}

	playQueueItem(){
		var selectedTracks = this.props.context_menu.data.selected_tracks;
		this.props.mopidyActions.changeTrack( selectedTracks[0].tlid );
		this.props.uiActions.hideContextMenu();
	}

	removeFromQueue(){
		var selected_tracks = this.props.context_menu.data.selected_tracks;
		var selected_tracks_tlids = [];
		for( var i = 0; i < selected_tracks.length; i++ ){
			selected_tracks_tlids.push( selected_tracks[i].tlid );
		}
		this.props.mopidyActions.removeTracks( selected_tracks_tlids );
		this.props.uiActions.hideContextMenu();
	}

	playItems(){
		var selected_tracks_uris = helpers.asURIs(this.props.context_menu.data.selected_tracks)
		this.props.mopidyActions.playURIs(selected_tracks_uris);
		this.props.uiActions.hideContextMenu();
	}

	playItemsNext(){
		var selected_tracks_uris = helpers.asURIs(this.props.context_menu.data.selected_tracks)
		this.props.mopidyActions.enqueueTracksNext(selected_tracks_uris);
		this.props.uiActions.hideContextMenu();
	}

	addToPlaylist(){
		var selected_tracks_uris = helpers.asURIs(this.props.context_menu.data.selected_tracks)
		this.props.uiActions.openModal( 'add_to_playlist', { tracks_uris: selected_tracks_uris } )
		this.props.uiActions.hideContextMenu();
	}

	addToQueue(){
		var selected_tracks_uris = helpers.asURIs(this.props.context_menu.data.selected_tracks)
		this.props.mopidyActions.enqueueTracks(selected_tracks_uris);
		this.props.uiActions.hideContextMenu();
	}

	addTracksToPlaylist( playlist_uri ){
		var selected_tracks_uris = helpers.asURIs(this.props.context_menu.data.selected_tracks)
		this.props.uiActions.addTracksToPlaylist( playlist_uri, selected_tracks_uris )
		this.props.uiActions.hideContextMenu();
	}

	removeFromPlaylist(){
		this.props.uiActions.removeTracksFromPlaylist( this.props.playlist.uri, this.props.context_menu.data.selected_tracks_indexes )
		this.props.uiActions.hideContextMenu();
	}

	copyURIs(){
		var uris = '';
		for( var i = 0; i < this.props.context_menu.data.selected_tracks.length; i++ ){
			if( i > 0 ) uris += ','
			uris += this.props.context_menu.data.selected_tracks[i].uri
		}
		console.log('Yet to be implemented', uris)
		this.props.uiActions.hideContextMenu()
	}

	closeAndDeselectTracks(){
		this.props.uiActions.hideContextMenu();
	}

	renderPlaylistSubmenu(){
		var playlists = []
		for( var i = 0; i < this.props.playlists.length; i++ ){
			if( this.props.playlists[i].can_edit ) playlists.push( this.props.playlists[i] )
		}

		return (			
			<div className="submenu">
				{
					playlists.map( playlist => {
						return (
							<span className="menu-item-wrapper" key={playlist.uri} >
								<a className="menu-item" onClick={ () => this.addTracksToPlaylist(playlist.uri) }>
									<span className="label">{ playlist.name }</span>
								</a>
							</span>
						)
					})
				}
			</div>
		)
	}

	renderItems(trigger){
		var items = []

		switch( this.props.context_menu.context ){

			case 'queue':
				items = [
					{ handleClick: 'playQueueItem', label: 'Play', icon: 'play' },
					{ handleClick: 'addToPlaylist', label: 'Add to playlist', icon: 'plus', playlists: true },
					{ handleClick: 'copyURIs', label: 'Copy URIs', icon: 'copy' },
					{ handleClick: 'removeFromQueue', label: 'Remove', icon: 'trash' }
				]
				break

			case 'editable-playlist':
				items = [
					{ handleClick: 'playItems', label: 'Play', icon: 'play' },
					{ handleClick: 'playItemsNext', label: 'Play next', icon: 'play' },
					{ handleClick: 'addToQueue', label: 'Add to queue', icon: 'plus' },
					{ handleClick: 'addToPlaylist', label: 'Add to playlist', icon: 'plus', playlists: true },
					{ handleClick: 'copyURIs', label: 'Copy URIs', icon: 'copy' },
					{ handleClick: 'removeFromPlaylist', label: 'Remove', icon: 'trash' }
				]
				break

			default:
				items = [
					{ handleClick: 'playItems', label: 'Play', icon: 'play' },
					{ handleClick: 'playItemsNext', label: 'Play next', icon: 'play' },
					{ handleClick: 'addToQueue', label: 'Add to queue', icon: 'plus' },
					{ handleClick: 'addToPlaylist', label: 'Add to playlist', icon: 'plus', playlists: true },
					{ handleClick: 'copyURIs', label: 'Copy URIs', icon: 'copy' }
				]
				break
		}

		var closeItem = (
			<span className="menu-item-wrapper cancel">
				<a className="menu-item" onClick={ (e) => this.closeAndDeselectTracks(e) }>
					<FontAwesome className="icon" fixedWidth name='close' />
					<span className="label">Cancel</span>
				</a>
			</span>
		)

		return (
			<div>
				{
					items.map((item, index) => {
						if( item.playlists ){
							return (
								<span key={item.handleClick} className="menu-item-wrapper has-submenu">
									<a className="menu-item" onClick={ (e) => this[item.handleClick](e) }>
										<FontAwesome className="icon" fixedWidth name={item.icon} />
										<span className="label">{ item.label }</span>
										<FontAwesome className="submenu-icon" name="caret-right" />
									</a>
									{ this.renderPlaylistSubmenu() }
								</span>
							)
						}else{
							return (
								<span className="menu-item-wrapper" key={item.handleClick}>
									<a className="menu-item" onClick={ (e) => this[item.handleClick](e) }>
										<FontAwesome className="icon" fixedWidth name={item.icon} />
										<span className="label">{ item.label }</span>
									</a>
								</span>
							)
						}
					})
				}
				{ trigger == 'touch' ? closeItem : null }
			</div>
		)
	}

	render(){
		if( !this.props.context_menu.show ) return null;

		var style = {
			left: this.props.context_menu.position_x,
			top: this.props.context_menu.position_y,
		}

		var className = 'context-menu'
		var trigger = 'click'
		if (this.props.trigger_override) {
			className += ' '+this.props.trigger_override
			trigger = this.props.trigger_override
		} else {
			className += ' '+this.props.context_menu.trigger
		}

		return (
			<div className={className} style={style}>
				{ this.renderItems(trigger) }
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		trigger_override: state.ui.trigger_override,
		context_menu: state.ui.context_menu,
		current_track: state.ui.current_track,
		current_tracklist: state.ui.current_tracklist,
		playlist: state.ui.playlist,
		playlists: state.ui.playlists
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(ContextMenu)
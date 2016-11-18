
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import TrackList from './TrackList'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class ContextMenu extends React.Component{

	constructor(props) {
		super(props);
		this.handleScroll = this.handleScroll.bind(this);
	}

	componentDidMount(){		
		window.addEventListener("scroll", this.handleScroll, false);
	}

	componentWillUnmount(){		
		window.removeEventListener("scroll", this.handleScroll, false);
	}

	handleScroll(){
		if( this.props.context_menu.show ){
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
		var selected_tracks = this.props.context_menu.data.selected_tracks;
		var selected_tracks_uris = [];
		for( var i = 0; i < selected_tracks.length; i++ ){
			selected_tracks_uris.push( selected_tracks[i].uri );
		}
		this.props.mopidyActions.playURIs(selected_tracks_uris);
		this.props.uiActions.hideContextMenu();
	}

	playItemsNext(){
		var selected_tracks = this.props.context_menu.data.selected_tracks;
		var selected_tracks_uris = [];
		for( var i = 0; i < selected_tracks.length; i++ ){
			selected_tracks_uris.push( selected_tracks[i].uri );
		}

		var current_track = this.props.current_track
		var current_track_index = -1
		for( var i = 0; i < this.props.current_tracklist.length; i++ ){
			if( this.props.current_tracklist[i].tlid == this.props.current_track.tlid ){
				current_track_index = i
				break
			}
		}

		var at_position = null
		if( current_track_index > -1 ) at_position = current_track_index + 1

		this.props.mopidyActions.enqueueTracks(selected_tracks_uris, at_position);
		this.props.uiActions.hideContextMenu();
	}

	addToQueue(){
		var selected_tracks = this.props.context_menu.data.selected_tracks;
		var selected_tracks_uris = [];
		for( var i = 0; i < selected_tracks.length; i++ ){
			selected_tracks_uris.push( selected_tracks[i].uri );
		}
		this.props.mopidyActions.enqueueTracks(selected_tracks_uris);
		this.props.uiActions.hideContextMenu();
	}

	addToPlaylist(){
		this.props.uiActions.openModal( 'AddToPlaylistModal', { track_indexes: this.props.context_menu.data.selected_tracks_indexes })
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
							<a 
								className="menu-item" 
								key={playlist.uri} 
								onClick={ () => {
									this.props.uiActions.addTracksToPlaylist( playlist.uri, this.props.context_menu.data.selected_tracks )
									this.props.uiActions.hideContextMenu() }
							}>
								{ playlist.name }
							</a>
						)
					})
				}
			</div>
		)
	}

	renderItems(){
		var items = [];

		switch( this.props.context_menu.context ){

			case 'queue':
				items = [
					{ handleClick: 'playQueueItem', label: 'Play' },
					{ handleClick: 'addToPlaylist', label: 'Add to playlist', playlists: true },
					{ handleClick: 'copyURIs', label: 'Copy URIs' },
					{ handleClick: 'removeFromQueue', label: 'Remove' }
				];
				break;

			case 'editable-playlist':
				items = [
					{ handleClick: 'playItems', label: 'Play' },
					{ handleClick: 'playItemsNext', label: 'Play next' },
					{ handleClick: 'addToQueue', label: 'Add to queue' },
					{ handleClick: 'addToPlaylist', label: 'Add to playlist', playlists: true },
					{ handleClick: 'copyURIs', label: 'Copy URIs' },
					{ handleClick: 'removeFromPlaylist', label: 'Remove' }
				];
				break;

			default:
				items = [
					{ handleClick: 'playItems', label: 'Play' },
					{ handleClick: 'playItemsNext', label: 'Play next' },
					{ handleClick: 'addToQueue', label: 'Add to queue' },
					{ handleClick: 'addToPlaylist', label: 'Add to playlist', playlists: true },
					{ handleClick: 'copyURIs', label: 'Copy URIs' }
				];
				break;
		}

		return (
			<div>
				{
					items.map((item, index) => {
						if( item.playlists ){
							return (
								<span key={item.handleClick} className="menu-item has-submenu">
									{ item.label }
									<FontAwesome name="caret-right" />
									{ this.renderPlaylistSubmenu() }
								</span>
							)
						}else{
							return <a className="menu-item" key={item.handleClick} onClick={ (e) => this[item.handleClick](e) }>{ item.label }</a>
						}
					})
				}
			</div>
		);
	}

	render(){
		if( !this.props.context_menu.show ) return null;

		var style = {
			left: this.props.context_menu.position_x,
			top: this.props.context_menu.position_y,
		}

		return (
			<div className="context-menu" style={style}>
				{ this.renderItems() }
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
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
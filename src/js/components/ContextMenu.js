
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import TrackList from './TrackList'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class ContextMenu extends React.Component{

	constructor(props) {
		super(props);
	}

	renderItems(){
		var items = [];

		switch( this.props.context_menu.context ){

			case 'queue':
				items = [
					{ handleClick: 'playQueueItem', label: 'Play' },
					{ handleClick: 'removeFromQueue', label: 'Remove' }
				];
				break;

			case 'editable-playlist':
				items = [
					{ handleClick: 'playItems', label: 'Play' },
					{ handleClick: 'playItemsNext', label: 'Play next' },
					{ handleClick: 'addToQueue', label: 'Add to queue' },
					{ handleClick: 'removeFromPlaylist', label: 'Remove' }
				];
				break;

			default:
				items = [
					{ handleClick: 'playItems', label: 'Play' },
					{ handleClick: 'playItemsNext', label: 'Play next' },
					{ handleClick: 'addToQueue', label: 'Add to queue' }
				];
				break;
		}

		return (
			<div>
				{
					items.map((item, index) => {
						return <a key={item.handleClick} onClick={ (e) => this[item.handleClick](e) }>{ item.label }</a>
					})
				}
			</div>
		);
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
		this.props.mopidyActions.playTracks(selected_tracks_uris);
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

	removeFromPlaylist(){
		this.props.uiActions.removeTracksFromPlaylist( this.props.context_menu.data.selected_tracks_indexes )
		this.props.uiActions.hideContextMenu();
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
		current_tracklist: state.ui.current_tracklist
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

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

	handleClick(e){
	}

	renderItems(){
		var items = [];

		switch( this.props.state.context ){

			case 'queue':
				items = [
					{ handleClick: 'playQueueItem', label: 'Play' },
					{ handleClick: 'removeFromQueue', label: 'Remove' }
				];
				break;

			case 'editable-playlist':
				items = [
					{ handleClick: 'playItems', label: 'Play' },
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
		var selectedTracks = this.props.state.data.selected_tracks;
		this.props.mopidyActions.changeTrack( selectedTracks[0].tlid );
		this.props.uiActions.hideContextMenu();
	}

	removeFromQueue(){
		var selectedTracks = this.props.state.data.selected_tracks;
		var selectedTracksTlids = [];
		for( var i = 0; i < selectedTracks.length; i++ ){
			selectedTracksTlids.push( selectedTracks[i].tlid );
		}
		this.props.mopidyActions.removeTracks( selectedTracksTlids );
		this.props.uiActions.hideContextMenu();
	}

	playItems(){
		var selectedTracks = this.props.state.data.selected_tracks;
		var selectedTracksUris = [];
		for( var i = 0; i < selectedTracks.length; i++ ){
			selectedTracksUris.push( selectedTracks[i].uri );
		}
		this.props.mopidyActions.playTracks(selectedTracksUris);
		this.props.uiActions.hideContextMenu();
	}

	playItemsNext(){
		var selectedTracks = this.props.state.data.selected_tracks;
		var selectedTracksUris = [];
		for( var i = 0; i < selectedTracks.length; i++ ){
			selectedTracksUris.push( selectedTracks[i].uri );
		}

		var current_tltrack = this.props.mopidy.current_tltrack;
		function isCurrentTlid( tltrack ){
			return ( tltrack.tlid == current_tltrack.tlid );
		}
		var currentTrack = this.props.mopidy.tracks.find( isCurrentTlid );
		var currentTrackIndex = this.props.mopidy.tracks.indexOf( currentTrack );

		this.props.mopidyActions.enqueueTracks(selectedTracksUris, currentTrackIndex+1);
		this.props.uiActions.hideContextMenu();
	}

	addToQueue(){
		var selectedTracks = this.props.state.data.selected_tracks;
		var selectedTracksUris = [];
		for( var i = 0; i < selectedTracks.length; i++ ){
			selectedTracksUris.push( selectedTracks[i].uri );
		}
		this.props.mopidyActions.enqueueTracks(selectedTracksUris);
		this.props.uiActions.hideContextMenu();
	}

	removeFromPlaylist(){
		console.log('removeFromPlaylist')
		this.props.uiActions.hideContextMenu();
	}

	render(){
		if( !this.props.state.show ) return null;

		var style = {
			left: this.props.state.position_x,
			top: this.props.state.position_y,
		}

		return (
			<div className="context-menu" style={style}>
				{ this.renderItems() }
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(ContextMenu)
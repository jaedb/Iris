
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Track from './Track'

import * as mopidyActions from '../services/mopidy/actions'
import * as uiActions from '../services/ui/actions'

class TrackList extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			tracks: this.keyifyTracks(this.props.tracks),
			lastSelectedTrack: false
		}

		this.handleKeyUp = this.handleKeyUp.bind(this)
	}

	componentWillMount(){
		window.addEventListener("keyup", this.handleKeyUp, false);
	}

	componentWillUnmount(){
		window.removeEventListener("keyup", this.handleKeyUp, false);
	}

	componentWillReceiveProps( nextProps ){
		this.setState({ tracks: this.keyifyTracks(nextProps.tracks) });
	}

	handleKeyUp(e){
		if( this.selectedTracks().length <= 0 ) return;

		switch(e.keyCode){
			
			case 13: // enter
				 this.playTracks();
				break;
			
			case 46: // delete
				 this.removeTracks();
				break;
		}
	}

	handleDoubleClick(e, index){
		if( this.props.context_menu.show ) this.props.uiActions.hideContextMenu()
		this.playTracks()
	}

	handleMouseDown(e, index){
		if( this.props.context_menu.show ) this.props.uiActions.hideContextMenu()

		if( !this.state.tracks[index].selected && !this.isRightClick(e) && !e.ctrlKey ) this.toggleTrackSelections(e, index)

		var selected_tracks = this.selectedTracks()
		this.props.uiActions.dragStart( e, this.props.context, selected_tracks, this.tracksIndexes(selected_tracks) )
	}

	handleMouseUp(e, index){

		// right-clicking on an un-highlighted track
		if( !this.state.tracks[index].selected && this.isRightClick(e) ){
			this.toggleTrackSelections(e, index)

		// selected track, regular click
		}else if( this.state.tracks[index].selected && !this.isRightClick(e) ){
			this.toggleTrackSelections(e, index)

		// ctrl key
		}else if( e.ctrlKey ){
			this.toggleTrackSelections(e, index)
		}

		if( this.props.dragger && this.props.dragger.active ){
		
			// if this tracklist handles sorting, handle it
			if( typeof(this.props.reorderTracks) !== 'undefined' ){
				var indexes = this.props.dragger.victims_indexes
				return this.props.reorderTracks( indexes, index );
			}
		}
	}

	handleContextMenu(e, index){
		var selected_tracks = this.selectedTracks()
		var data = {
			selected_tracks: selected_tracks,
			selected_tracks_indexes: this.tracksIndexes( selected_tracks )
		}
		this.props.uiActions.showContextMenu( e, this.props.context, data )
	}

	toggleTrackSelections(e, index){
		var tracks = this.state.tracks

		if( e.ctrlKey ){

			tracks[index].selected = !tracks[index].selected

		}else if( e.shiftKey ){

			if( this.state.lastSelectedTrack < index ){
				var start = this.state.lastSelectedTrack
				var end = index
			}else{
				var start = index
				var end = this.state.lastSelectedTrack
			}

			for( var i = start; i <= end; i++ ){
				tracks[i].selected = true
			}

		}else{

			for( var i = 0; i < tracks.length; i++ ){
				tracks[i].selected = false
			}

			tracks[index].selected = !tracks[index].selected
		}

		this.setState({ tracks: tracks, lastSelectedTrack: index })
	}

	isRightClick(e){
		if( 'which' in e ) 
			return e.which == 3
		if( 'button' in e ) 
			return e.button == 2
		return false
	}

	keyifyTracks( tracks ){
		for( var i = 0; i < tracks.length; i++ ){
			tracks[i] = Object.assign({}, tracks[i], { key: i+'_'+tracks[i].uri })
		}
		return tracks
	}

	selectedTracks(){
		function isSelected( track ){
			return ( typeof(track.selected) !== 'undefined' && track.selected );
		}
		return this.state.tracks.filter(isSelected)
	}

	tracksIndexes(tracks){
		var indexes = []
		for( var i = 0; i < tracks.length; i++ ){
			indexes.push( this.props.tracks.indexOf(tracks[i]))
		}
		return indexes
	}

	playTracks(){

		var tracks = this.selectedTracks();

		// if we've got a specific action, run it
		if( typeof(this.props.playTracks) !== 'undefined' ){
			return this.props.playTracks( tracks );
		}

		// default to playing a bunch of uris
		var uris = [];
		for( var i = 0; i < tracks.length; i++ ){
			uris.push( tracks[i].uri )
		}
		return this.props.mopidyActions.playTracks( uris )
	}

	removeTracks(){
		
		// if this tracklist handles removal, handle it
		if( typeof(this.props.removeTracks) !== 'undefined' ){
			var tracks = this.selectedTracks();
			var tracks_indexes = this.tracksIndexes(tracks);
			return this.props.removeTracks( tracks_indexes );
		}

		// by default, do nothing
	}

	render(){
		if( !this.state.tracks ) return null

		let self = this;
		return (
			<ul>
				<li className="list-item header track">
					<span className="col name">Name</span>
					<span className="col artists">Artists</span>
					<span className="col album">Album</span>
					<span className="col duration">Length</span>
				</li>
				{
					this.state.tracks.map(
						(track, index) => {
							return <Track
									show_source_icon={ this.props.show_source_icon }
									key={track.key} 
									track={track} 
									handleDoubleClick={ e => self.handleDoubleClick(e, index)}
									handleClick={ e => self.handleClick(e, index)}
									handleMouseUp={ e => self.handleMouseUp(e, index)}
									handleMouseDown={ e => self.handleMouseDown(e, index)}
									handleContextMenu={ e => self.handleContextMenu(e, index)} />
						}
					)
				}
			</ul>
		);
	}
}


/**
 * Export our component
 *
 * We also integrate our global store, using connect()
 **/

const mapStateToProps = (state, ownProps) => {
	return {
		dragger: state.ui.dragger,
		current_track: state.ui.current_track,
		context_menu: state.ui.context_menu
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(TrackList)
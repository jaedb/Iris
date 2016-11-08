
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import * as mopidyActions from '../services/mopidy/actions'
import * as uiActions from '../services/ui/actions'

import Track from './Track'

class TrackList extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			tracks: this.props.tracks,
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

	componentWillReceiveProps( nextProps ){
		this.setState({ tracks: nextProps.tracks });
	}

	handleClick( e, index ){
		if( this.props.ui.context_menu.show ) this.props.uiActions.hideContextMenu();

		var tracks = this.state.tracks;

		if( e.ctrlKey ){

			tracks[index].selected = !tracks[index].selected;

		}else if( e.shiftKey ){

			if( this.state.lastSelectedTrack < index ){
				var start = this.state.lastSelectedTrack;
				var end = index;
			}else{
				var start = index;
				var end = this.state.lastSelectedTrack;
			}

			for( var i = start; i <= end; i++ ){
				tracks[i].selected = true;
			}

		}else{

			for( var i = 0; i < tracks.length; i++ ){
				tracks[i].selected = false;
			}
			tracks[index].selected = !tracks[index].selected;
		}

		this.setState({ tracks: tracks, lastSelectedTrack: index });
	}

	handleDoubleClick( e, index ){
		if( this.props.ui.context_menu.show ) this.props.uiActions.hideContextMenu();
		this.playTracks()
	}

	handleContextMenu( e, index ){
		var data = {
			selected_tracks: this.selectedTracks()
		}
		this.props.uiActions.showContextMenu( e, this.props.context, data )
	}

	selectedTracks(){
		function isSelected( track ){
			return ( typeof(track.selected) !== 'undefined' && track.selected );
		}
		return this.state.tracks.filter(isSelected)
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

		var tracks = this.selectedTracks();
		
		// if we've got a specific action, run it
		if( typeof(this.props.removeTracks) !== 'undefined' ){
			return this.props.removeTracks( tracks );
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

							// flatten nested track objects (as in the case of TlTracks)
							if( typeof(track.track) !== 'undefined' ){

								// see if we're the current tlTrack
								// TODO: figure out why this isn't fired when the tracklist changes
								if( self.props.mopidy.current_tltrack && self.props.mopidy.current_tltrack.tlid == track.tlid ){
									track.playing = true;
								}else{
									track.playing = false;
								}

								for( var property in track.track ){
									if( track.track.hasOwnProperty(property) ){
										track[property] = track.track[property]
									}
								}
							}
							return <Track
									show_source_icon={ this.props.show_source_icon }
									key={index+'_'+track.uri} 
									track={track} 
									handleDoubleClick={(e) => self.handleDoubleClick(e, index)}
									handleClick={(e) => self.handleClick(e, index)}
									handleContextMenu={(e) => self.handleContextMenu(e, index)} />
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
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(TrackList)
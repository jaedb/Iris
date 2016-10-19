
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import * as actions from '../services/mopidy/actions'

import Track from './Track'

class TrackList extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			tracks: this.props.tracks,
			lastSelectedTrack: false
		}
	}

	handleClick( e, index ){
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

	selectedTracks(){
		function isSelected( track ){
			return ( typeof(track.selected) !== 'undefined' && track.selected );
		}
		return this.state.tracks.filter(isSelected)
	}

	handleDoubleClick( e, index ){
		var tracks = this.state.tracks;
		this.props.playTrack( tracks[index] )
	}

	playTracks(){
		this.props.playTracks( this.selectedTracks() )
	}

	removeTracks(){
		this.props.removeTracks( this.selectedTracks() )
	}

	componentWillReceiveProps( nextProps ){
		this.setState({ tracks: nextProps.tracks });
	}

	render(){
		let self = this;
		if( this.state.tracks ){
			return (
				<div>
					<ul>
						{
							this.state.tracks.map(
								(track, index) => {

									// flatten nested track objects (as in the case of TlTracks)
									if( typeof(track.track) !== 'undefined' ){
										for( var property in track.track ){
											if( track.track.hasOwnProperty(property) ){
												track[property] = track.track[property]
											}
										}
									}
									return <Track
											key={index+'_'+track.uri} 
											track={track} 
											handleDoubleClick={(e) => self.handleDoubleClick(e, index)}
											handleClick={(e) => self.handleClick(e, index)} />
								}
							)
						}
					</ul>
					<button onClick={() => this.removeTracks()}>Delete selected</button>
					<button onClick={() => this.playTracks()}>Play selected</button>
				</div>
			);
		}
		return null;
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
		actions: bindActionCreators(actions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(TrackList)
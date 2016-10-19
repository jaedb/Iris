
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import FontAwesome from 'react-fontawesome'
import TrackList from '../components/TrackList'
import Track from '../components/Track'
import Player from '../components/Player'
import ArtistList from '../components/ArtistList'
import AlbumLink from '../components/AlbumLink'
import * as actions from '../services/mopidy/actions'

class Queue extends React.Component{

	constructor(props) {
		super(props);
	}

	renderTrackInFocus(){
		if( this.props.mopidy && this.props.mopidy.trackInFocus ){
			return (
				<div>
					<div>{ this.props.mopidy.trackInFocus.track.name }</div>
					<div><ArtistList artists={ this.props.mopidy.trackInFocus.track.artists } /></div>
					<div><AlbumLink album={ this.props.mopidy.trackInFocus.track.album } /></div>
				</div>
			);
		}
		return null;
	}

	renderTrackList(){
		if( this.props.mopidy && this.props.mopidy.tracks ){
			return (
				<TrackList
					tracks={this.props.mopidy.tracks} 
					removeTracks={ tracks => this.removeTracks( tracks ) }
					playTracks={ null }
					playTrack={ track => this.playTrack( track ) }
					/>
			);
		}
		return null;
	}

	removeTracks( tracks ){
		var tlids = [];
		for( var i = 0; i < tracks.length; i++ ){
			tlids.push( tracks[i].tlid )
		}
		this.props.actions.removeTracks( tlids )
	}

	playTrack( track ){		
		this.props.actions.changeTrack( track.tlid )
	}

	render(){
		return (
			<div>
				<h3>Now playing</h3>
				{ this.renderTrackInFocus() }
				<Player />
				<h4>Other tracks</h4>
				{ this.renderTrackList() }
			</div>
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
		actions: bindActionCreators(actions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Queue)
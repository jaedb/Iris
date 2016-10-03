
import Mopidy from 'mopidy'
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import * as actions from '../actions/mopidy'


/**
 * Mopidy service
 *
 * Handles internal requests and passes them on to our connection
 **/
class MopidyService extends React.Component{

	constructor( props ){
		super(props)

		this.connection = new Mopidy({
			webSocketUrl: "ws://music.james:6680/mopidy/ws",
			callingConvention: 'by-position-or-by-name'
		});

		this.connection.on( (type, data) => this.handleMessage( type, data ) );
	}

	handleMessage( type, data ){
		switch( type ){

			case 'state:online':
				this.props.actions.updateStatus( true );
				this.getState();
				this.getTracklist();
				this.getTrackInFocus();
				this.getVolume();
				this.getConsume();
				//this.getShuffle();
				//this.getRandom();
				break;

			case 'state:offline':
				this.props.actions.updateStatus( false );
				break;

			case 'event:tracklistChanged':
				this.getTracklist();
				break;

			//case 'event:trackPlaybackEnded':
			case 'event:playbackStateChanged':
			case 'event:trackPlaybackStarted':
				this.getTrackInFocus();
				this.getState();
				break;

			case 'event:volumeChanged':
				this.props.actions.updateVolume( data.volume );
				break;

			case 'event:optionsChanged':
				this.getConsume();
				//this.getShuffle();
				//this.getRandom();
				break;

			default:
				//console.log( 'MopidyService: Unhandled event', type, message );
		}
	}

	getVolume(){
		let self = this;
		this.connection.playback.getVolume()
			.then( function( volume ){
				self.props.actions.updateVolume( volume );
			});
	}

	setVolume( volume ){
		this.props.actions.updateVolume( volume );
	}

	getTracklist(){
		let self = this;
		this.connection.tracklist.getTlTracks()
			.then( function( tracks ){
				self.props.actions.updateTracklist( tracks );
			});
	}

	getTrackInFocus(){
		let self = this;
		this.connection.playback.getCurrentTlTrack()
			.then( function( tltrack ){
				self.props.actions.updateTrackInFocus( tltrack );
			});
	}

	getState(){
		let self = this;
		this.connection.playback.getState()
			.then( function( state ){
				self.props.actions.updateState( state );
			});
	}

	getConsume(){
		let self = this;
		this.connection.tracklist.getConsume()
			.then( function( consume ){
				self.props.actions.updateConsume( consume );
			});
	}

	render(){
		console.log( this.props.mopidy );
		var playButton = <a onClick={() => this.connection.playback.play()}>Play</a>;
		if( this.props.mopidy.state == 'playing' ){
			playButton = <a onClick={() => this.connection.playback.pause()}>Pause</a>;
		}

		var consumeButton = <a onClick={() => this.connection.tracklist.setConsume(true)}>Consume</a>;
		if( this.props.mopidy.consume ){
			consumeButton = <a onClick={() => this.connection.tracklist.setConsume(false)}>Un-Consume</a>;
		}

		return (
			<div>
				{ playButton }
				<a onClick={() => this.connection.playback.previous()}>Previous</a>
				<a onClick={() => this.connection.playback.next()}>Next</a>
				{ consumeButton }
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

export default connect(mapStateToProps, mapDispatchToProps)(MopidyService)
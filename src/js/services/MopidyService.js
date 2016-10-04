
import Mopidy from 'mopidy'
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

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
				this.get( 'playback', 'State' );
				this.get( 'playback', 'Volume' );
				this.get( 'tracklist', 'Consume' );
				this.get( 'tracklist', 'TlTracks' );
				this.get( 'playback', 'CurrentTlTrack' );
				//this.getShuffle();
				//this.getRandom();
				break;

			case 'state:offline':
				this.props.actions.updateStatus( false );
				break;

			case 'event:tracklistChanged':
				this.get( 'tracklist', 'TlTracks' );
				break;

			//case 'event:trackPlaybackEnded':
			case 'event:playbackStateChanged':
			case 'event:trackPlaybackStarted':
				this.get( 'playback', 'State' );
				this.get( 'playback', 'CurrentTlTrack' );
				break;

			case 'event:volumeChanged':
				this.props.actions.updateVolume( data.volume );
				break;

			case 'event:optionsChanged':
				this.get( 'tracklist', 'Consume' );
				//this.getShuffle();
				//this.getRandom();
				break;

			default:
				//console.log( 'MopidyService: Unhandled event', type, message );
		}
	}

	get( model, property ){
		console.log('MopidyServie: getting '+model+'.'+property);
		let self = this;
		this.connection[model]['get'+property]()
			.then( function( response ){
				self.props.actions['update'+property]( response );
			});
	}

	set( model, property ){
		console.log('MopidyServie: setting '+model+'.'+property);
		let self = this;
		this.connection[model]['get'+property]()
			.then( function( response ){
				self.props.actions['update'+property]( response );
			});
	}

	render(){
		console.log( this.props.mopidy );
		var playButton = <a onClick={() => this.connection.playback.play()}><FontAwesome name="play" /> </a>
		if( this.props.mopidy.state == 'playing' ){
			playButton = <a onClick={() => this.connection.playback.pause()}><FontAwesome name="pause" /> </a>
		}

		var consumeButton = <a onClick={() => this.connection.tracklist.setConsume(true)}>Consume </a>
		if( this.props.mopidy.consume ){
			consumeButton = <a onClick={() => this.connection.tracklist.setConsume(false)}>Un-Consume </a>
		}

		return (
			<div>
				{ playButton }
				<a onClick={() => this.connection.playback.previous()}><FontAwesome name="step-backward" /> </a>
				<a onClick={() => this.connection.playback.next()}><FontAwesome name="step-forward" /> </a>
				{ consumeButton }
				{ this.props.mopidy.volume }
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
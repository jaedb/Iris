
import Mopidy from 'mopidy'
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import * as actions from '../services/mopidy/actions'


/**
 * Mopidy service
 *
 * Handles internal requests and passes them on to our connection
 **/
class MopidyService extends React.Component{

	constructor( props ){
		super(props);
/*
		this.connection = new Mopidy({
			webSocketUrl: "ws://tv.barnsley.nz:6680/mopidy/ws",
			//webSocketUrl: "ws://music.james:6680/mopidy/ws",
			callingConvention: 'by-position-or-by-name'
		});

		this.connection.on( (type, data) => this.handleMessage( type, data ) );
		*/
	}

	componentDidMount(){

		this.props.actions.connect();
	}

	handleMessage( type, data ){
		switch( type ){

			case 'state:online':
				this.props.actions.updateStatus( true );
				this.get( 'playback', 'State' );
				this.get( 'playback', 'Volume' );
				this.get( 'tracklist', 'Consume' );
				this.get( 'tracklist', 'Random' );
				this.get( 'tracklist', 'Repeat' );
				this.get( 'tracklist', 'TlTracks' );
				this.get( 'playback', 'CurrentTlTrack' );
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
				this.get( 'tracklist', 'Random' );
				this.get( 'tracklist', 'Repeat' );
				break;

			default:
				//console.log( 'MopidyService: Unhandled event', type, message );
		}
	}


	/**
	 * Get something from Mopidy
	 *
	 * Sends request to Mopidy server, and updates our local storage on return
	 * @param string model Mopidy model (playback, tracklist, etc)
	 * @param string property the property to get (TlTracks, Consume, etc)
	 **/
	get( model, property ){
		console.log('MopidyService: '+model+'.get'+property);
		let self = this;
		this.connection[model]['get'+property]()
			.then(
				function( response ){
					self.props.actions['update'+property]( response );
				},
				function( error ){
					console.error( error );
				}
			);
	}


	/**
	 * Set something in Mopidy
	 *
	 * Sends request to Mopidy server, and updates our local storage on return
	 * @param string model Mopidy model (playback, tracklist, etc)
	 * @param string property the property to get (TlTracks, Consume, etc)
	 * @param mixed value
	 **/
	set( model, property, value ){
		console.log('MopidyServie: '+model+'.set'+property, value);
		let self = this;
		this.connection[model]['set'+property]( value )
			.then(
				function( response ){
					self.props.actions['update'+property]( response );
				},
				function( error ){
					console.error( error );
				}
			);
	}

	changeTlTrack( tlid ){
		console.log('Changing track to '+ tlid)
	}


	render(){
		console.log( this.props.mopidy );
		var playButton = <a onClick={() => this.connection.playback.play()}><FontAwesome name="play" /> </a>
		if( this.props.mopidy.state == 'playing' ){
			playButton = <a onClick={() => this.connection.playback.pause()}><FontAwesome name="pause" /> </a>
		}

		var consumeButton = <a onClick={() => this.set('tracklist', 'Consume', [true])}>Consume </a>
		if( this.props.mopidy.consume ){
			consumeButton = <a onClick={() => this.set('tracklist', 'Consume', [false])}>Un-Consume </a>
		}

		var randomButton = <a onClick={() => this.set('tracklist', 'Random', [true])}>Random </a>
		if( this.props.mopidy.random ){
			randomButton = <a onClick={() => this.set('tracklist', 'Random', [false])}>Un-Random </a>
		}

		var repeatButton = <a onClick={() => this.set('tracklist', 'Repeat', [true])}>Repeat </a>
		if( this.props.mopidy.repeat ){
			repeatButton = <a onClick={() => this.set('tracklist', 'Repeat', [false])}>Un-Repeat </a>
		}

		return (
			<div>
				{ playButton }
				<a onClick={() => this.connection.playback.previous()}><FontAwesome name="step-backward" /> </a>
				<a onClick={() => this.connection.playback.next()}><FontAwesome name="step-forward" /> </a>
				{ consumeButton }
				{ randomButton }
				{ repeatButton }
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
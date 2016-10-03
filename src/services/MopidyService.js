
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
			webSocketUrl: "ws://music.barnsley.nz:6680/mopidy/ws",
			callingConvention: 'by-position-or-by-name'
		});

		this.connection.on( (type, data) => this.handleMessage( type, data ) );
	}

	handleMessage( type, data ){
		switch( type ){

			case 'state:online':
				this.props.actions.updateStatus( true );
				this.getTracklist();
				this.getTrackInFocus();
				this.getVolume();
				break;

			case 'state:offline':
				this.props.actions.updateStatus( false );
				break;

			case 'event:tracklistChanged':
				this.getTracklist();
				break;

			case 'event:volumeChanged':
				this.props.actions.updateVolume( data.volume );
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

	render(){
		return null
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
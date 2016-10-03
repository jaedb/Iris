
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
				this.updateStatus(true);
				this.getTracklist();
				this.getVolume();
				break;

			case 'event:tracklistChanged':
				this.getTracklist();
				break;

			case 'event:volumeChanged':
				this.props.actions.volumeChanged(data.volume);
				break;

			default:
				//console.log( 'MopidyService: Unhandled event', type, message );
		}
	}

	updateStatus( online = false ){
		this.props.actions.updateStatus( online );
	}

	getVolume(){
		let self = this;
		this.connection.playback.getVolume()
			.then( function(volume){
				self.props.actions.volumeChanged(volume);
			});
	}

	getTracklist(){
		let self = this;
		this.connection.tracklist.getTlTracks()
			.then( function(tracks){
				self.props.actions.updateTracklist(tracks);
			});
	}

	render(){
		return <pre>{ JSON.stringify(this.props.mopidy, null, 2) }</pre>;
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

import React, { PropTypes } from 'react'
import * as actions from '../actions/spotifyActions'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

class SpotifyService extends React.Component{

	constructor(props){
		super(props);
		let self = this;
		this.urlBase = 'https://api.spotify.com/v1/';
		console.log('SpotifyProvider > constructor')

		// listen for incoming messages from the authorization iframe
		// this is triggered when authentication is granted from the popup
		window.addEventListener('message', function(event){
			if( !/^https?:\/\/jamesbarnsley\.co\.nz/.test(event.origin) ) return false;
			self.handleMessage( event );
		}, false);
	}

	handleMessage( event ){
		
		// convert to json
		var data = JSON.parse(event.data);

		// fire event
		this.props.actions.authorizeSpotifySuccess( data );
	}

	test(){
		console.log('test stuff');
	}

	getAlbum( albumid ){
	    return $.when(
	        $.ajax({
				method: 'GET',
				cache: true,
				url: this.urlBase+'albums/'+albumid
	        })
	    );
	}

	render(){

		if( this.props.config.authorizingSpotify ){
			var src = '//jamesbarnsley.co.nz/spotmop.php?action=authorize&app='+location.protocol+'//'+window.location.host;
			return (
				<div>
					Authorizing...
					<iframe id="authorization-frame" src={src}></iframe>
				</div>
			);
		}

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

export default connect(mapStateToProps, mapDispatchToProps)(SpotifyService)
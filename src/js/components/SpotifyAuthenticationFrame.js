
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { createStore, bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import * as actions from '../services/spotify/actions'

class SpotifyAuthenticationFrame extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){

		let self = this;

		// listen for incoming messages from the authorization iframe
		// this is triggered when authentication is granted from the popup
		window.addEventListener('message', function(event){
			
			// only allow incoming data from our authorized authenticator proxy
			if( !/^https?:\/\/jamesbarnsley\.co\.nz/.test(event.origin) ) return false;
			
			var data = JSON.parse(event.data);
			self.props.actions.completeAuthorization( data );

		}, false);
	}

	renderAuthorizeButton(){
		if( this.props.spotify.authorizing ){
			return (
				<span>
					<FontAwesome name="circle-o-notch" spin />
					<span>Authorizing...</span>
				</span>
			);
		}else if( this.props.spotify.authorized ){
			return (
				<button onClick={() => this.props.actions.removeAuthorization()}>Log out</button>
			);
		}else{
			return (
				<button onClick={() => this.props.actions.startAuthorization()}>Log in</button>
			);
		}
	}

	render(){
		var src = '//jamesbarnsley.co.nz/spotmop.php?action=frame';
		if( this.props.spotify.authorizing ){
			src = '//jamesbarnsley.co.nz/spotmop.php?action=authorize&app='+location.protocol+'//'+window.location.host;
		}
		return (
			<div>
				{ this.renderAuthorizeButton() }
				<iframe src={src} style={{ display: 'none' }}></iframe>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		actions: bindActionCreators(actions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(SpotifyAuthenticationFrame)

import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { createStore, bindActionCreators } from 'redux'
import ReactGA from 'react-ga'

import FontAwesome from 'react-fontawesome'
import Thumbnail from './Thumbnail'

import * as uiActions from '../services/ui/actions'
import * as spotifyActions from '../services/spotify/actions'

class SpotifyAuthenticationFrame extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			frameUrl: '//jamesbarnsley.co.nz/auth.php?action=frame',
			authorizing: false
		}
	}

	componentDidMount(){

		let self = this;

		// listen for incoming messages from the authorization iframe
		// this is triggered when authentication is granted from the popup
		window.addEventListener('message', function(event){
			
			if(event.data == 'closed'){
				self.setState({
					frameUrl: '//jamesbarnsley.co.nz/auth.php?action=frame',
					authorizing: false
				})	
			}else if(event.data == 'blocked'){
				self.props.uiActions.createNotification('Popup blocked. Please allow popups and try again.','bad')
			}else{
				
				// only allow incoming data from our authorized authenticator proxy
				if( !/^https?:\/\/jamesbarnsley\.co\.nz/.test(event.origin) ) return false;
				
				var data = JSON.parse(event.data);
				self.props.spotifyActions.authorizationGranted( data );
				self.props.spotifyActions.getMe();

				// and turn off our authorizing switch
				self.setState({
					frameUrl: '//jamesbarnsley.co.nz/auth.php?action=frame',
					authorizing: false
				})				
			}

		}, false);
	}

	startAuthorization(){
		this.setState({
			frameUrl: '//jamesbarnsley.co.nz/auth.php?action=authorize&app='+location.protocol+'//'+window.location.host,
			authorizing: true
		})
	}

	renderAuthorizeButton(){
		if( this.state.authorizing ){
			return (
				<button disabled>
					<FontAwesome name="circle-o-notch" spin />
					&nbsp;
					Authorizing...
				</button>
			);
		}else if( this.props.authorized ){
			return (
				<button className="destructive" onClick={() => this.props.spotifyActions.authorizationRevoked()}>Log out</button>
			);
		}else{
			return (
				<button className="primary" onClick={() => this.startAuthorization()}>Log in</button>
			);
		}
	}

	renderRefreshButton(){
		if( !this.props.authorized ) return null;

		if( this.props.refreshing_token ){
			return (
				<button disabled>
					<FontAwesome name="circle-o-notch" spin />
					&nbsp;
					Refreshing...
				</button>
			);
		}else{
			return (
				<button onClick={() => this.props.spotifyActions.refreshingToken()}>Refresh token</button>
			);
		}
	}

	render(){
		return (
			<span>
				{ this.renderAuthorizeButton() }
				&nbsp;&nbsp;
				{ this.renderRefreshButton() }
				<iframe src={this.state.frameUrl} style={{ display: 'none' }}></iframe>
			</span>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		authorized: state.spotify.authorized,
		authorizing: state.spotify.authorizing,
		refreshing_token: state.spotify.refreshing_token
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(SpotifyAuthenticationFrame)
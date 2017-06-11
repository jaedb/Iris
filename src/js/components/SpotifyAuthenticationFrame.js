
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
			frameUrl: this.props.authorization_url+'?action=frame',
			authorizing: false
		}
	}

	componentDidMount(){

		let self = this;

		// Listen for incoming messages from the authorization iframe
		// This is triggered when the popup posts a message, which is then passed to
		// the iframe, and then passed on to the parent frame (our application)
		window.addEventListener('message', function(event){
				
			// only allow incoming data from our authorized authenticator proxy
			var authorization_domain = self.props.authorization_url.substring(0,self.props.authorization_url.indexOf('/',8))
			if (event.origin != authorization_domain){
				self.props.uiActions.createNotification('Authorization failed. '+event.origin+' is not the configured authorization_url.','bad')
				return false
			}
			
			// Window prematurely closed
			if (event.data == 'closed'){
				self.setState({
					frameUrl: self.props.authorization_url+'?action=frame',
					authorizing: false
				})

			// Popup was blocked by the browser
			} else if (event.data == 'blocked'){
				self.props.uiActions.createNotification('Popup blocked. Please allow popups and try again.','bad')
				self.setState({
					frameUrl: self.props.authorization_url+'?action=frame',
					authorizing: false
				})

			} else {
				var data = JSON.parse(event.data);

				// Spotify bounced with an error
				if (typeof(data.error) !== 'undefined'){
					self.props.uiActions.createNotification(data.error,'bad')

				// No errors? We're in!
				} else {
					self.props.spotifyActions.authorizationGranted(data);
					self.props.spotifyActions.getMe();			
				}

				// Turn off our authorizing switch
				self.setState({
					frameUrl: self.props.authorization_url+'?action=frame',
					authorizing: false
				})	
			}

		}, false);
	}

	startAuthorization(){
		this.setState({
			frameUrl: this.props.authorization_url+'?action=authorize&app='+location.protocol+'//'+window.location.host,
			authorizing: true
		})
	}

	renderAuthorizeButton(){
		if( this.state.authorizing ){
			return (
				<button className="working">
					Authorizing...
				</button>
			)
		}else if( this.props.authorized ){
			return (
				<button className="destructive" onClick={() => this.props.spotifyActions.authorizationRevoked()}>Log out</button>
			)
		}else{
			return (
				<button className="primary" onClick={() => this.startAuthorization()}>Log in</button>
			)
		}
	}

	renderRefreshButton(){
		if (!this.props.authorized) return null

		if (this.props.refreshing_token){
			return (
				<button className="working">
					Refreshing...
				</button>
			);
		} else {
			return (
				<button onClick={() => this.props.spotifyActions.refreshingToken()}>Force token refresh</button>
			)
		}
	}

	render(){
		return (
			<span>
				<iframe src={this.state.frameUrl} style={{ display: 'none' }}></iframe>
				{ this.renderAuthorizeButton() }
				{ this.renderRefreshButton() }
			</span>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		authorization_url: (state.ui.config && state.ui.config.authorization_url ? state.ui.config.authorization_url : 'https://jamesbarnsley.co.nz/auth.php'),
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
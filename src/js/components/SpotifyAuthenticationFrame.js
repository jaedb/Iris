
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
			authorizing: false
		}
	}

	componentDidMount(){

		let self = this;

		// Listen for incoming messages from the authorization iframe
		// This is triggered when the popup posts a message, which is then passed to
		// the iframe, and then passed on to the parent frame (our application)
		window.addEventListener('message', function(event){
			self.handleMessage(event)
		}, false);
	}

	handleMessage(event){

		var data = JSON.parse(event.data)
				
		// Only allow incoming data from our authorized authenticator proxy
		var authorization_domain = this.props.authorization_url.substring(0,this.props.authorization_url.indexOf('/',8))
		if (event.origin != authorization_domain){
			this.props.uiActions.createNotification('Authorization failed. '+event.origin+' is not the configured authorization_url.','bad')
			return false
		}

		// Spotify bounced with an error
		if (typeof(data.error) !== 'undefined'){
			this.props.uiActions.createNotification(data.error,'bad')

		// No errors? We're in!
		} else {
			this.props.spotifyActions.authorizationGranted(data)
			this.props.spotifyActions.getMe()
		}

		// Turn off our authorizing switch
		this.setState({authorizing: false})	
	}

	startAuthorization(){

		var self = this
		this.setState({authorizing: true})

		// Open an authentication request window (to spotify)
		var url = this.props.authorization_url+'?action=authorize'
		var scopes = [
			'playlist-modify-private',
			'playlist-modify-public',
			'playlist-read-private',
			'playlist-modify-private',
			'user-library-read',
			'user-library-modify',
			'user-follow-modify',
			'user-follow-read',
			'user-top-read',
			'user-read-currently-playing',
			'user-read-playback-state'
		]
		var popup = window.open(url+'&scope='+scopes.join('%20'),"popup","height=680,width=400");

		// Start timer to check our popup's state
		var timer = setInterval(checkPopup, 1000);
        function checkPopup(){

        	// Popup has been closed
            if (typeof(popup) !== 'undefined' && popup){
                if (popup.closed){
					self.setState({authorizing: false})
                    clearInterval(timer);
                }

            // Popup does not exist, so must have been blocked
            } else {
				self.props.uiActions.createNotification('Popup blocked. Please allow popups and try again.','bad')
				self.setState({authorizing: false})
                clearInterval(timer);
            }
        }
	}

	renderAuthorizeButton(){
		if (this.state.authorizing){
			return (
				<button className="working">
					Authorizing...
				</button>
			)
		} else if (this.props.authorized){
			return (
				<button className="destructive" onClick={() => this.props.spotifyActions.authorizationRevoked()}>Log out</button>
			)
		} else {
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
					Force token refresh
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
				{ this.renderAuthorizeButton() }
				{ this.renderRefreshButton() }
			</span>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		authorization_url: state.ui.config.authorization_url,
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

import React from 'react'
import { connect } from 'react-redux'
import { createStore, bindActionCreators } from 'redux'
import ReactGA from 'react-ga'

import Thumbnail from '../Thumbnail'

import * as uiActions from '../../services/ui/actions'
import * as lastfmActions from '../../services/lastfm/actions'

class LastfmAuthenticationFrame extends React.Component{

	constructor(props){
		super(props);

		this.state = {
			authorizing: false
		}
	}

	componentDidMount(){
		let self = this;

		// Listen for incoming messages from the authorization popup
		window.addEventListener('message', function(event){
			try {
				var data = JSON.parse(event.data);
			} catch (e){
				console.error("Failed to parse JSON", e, event);
				return;
			}

			// Only digest messages relevant to us
			if (data.origin == 'auth_lastfm'){
				self.handleMessage(event, data);
			}
		}, false);
	}

	handleMessage(event, data){
				
		// Only allow incoming data from our authorized authenticator proxy
		var authorization_domain = this.props.authorization_url.substring(0,this.props.authorization_url.indexOf('/',8))
		if (event.origin != authorization_domain){
			this.props.uiActions.createNotification({content: 'Authorization failed. '+event.origin+' is not the configured authorization_url.', type: 'bad'});
			return false
		}

		// Bounced with an error
		if (data.error !== undefined){
			this.props.uiActions.createNotification({content: data.message, type: 'bad'});

		// No errors? We're in!
		} else {
			this.props.lastfmActions.authorizationGranted(data)
			this.props.lastfmActions.getMe()
		}

		// Turn off our authorizing switch
		this.setState({authorizing: false});
	}

	startAuthorization(){

		var self = this
		this.setState({authorizing: true})

		// Open an authentication request window
		var url = this.props.authorization_url+'?action=authorize'
		var popup = window.open(url,"popup","height=580,width=350");
		popup.name = "LastfmAuthenticationWindow";

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
				self.props.uiActions.createNotification({content: 'Popup blocked. Please allow popups and try again.', type: 'bad'});
				self.setState({authorizing: false})
                clearInterval(timer);
            }
        }
	}

	render(){
		if (this.state.authorizing){
			return (
				<button className="button button--working">
					Authorizing...
				</button>
			)
		} else if (this.props.authorization){
			return (
				<button className="button button--destructive" onClick={() => this.props.lastfmActions.revokeAuthorization()}>Log out</button>
			)
		} else {
			return (
				<button className="button button--primary" onClick={() => this.startAuthorization()}>Log in</button>
			)
		}
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		authorization_url: state.lastfm.authorization_url,
		authorization: state.lastfm.authorization,
		authorizing: state.lastfm.authorizing
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		lastfmActions: bindActionCreators(lastfmActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LastfmAuthenticationFrame)
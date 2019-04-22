
import React from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import * as helpers from '../../helpers';

import * as uiActions from '../../services/ui/actions'
import * as geniusActions from '../../services/genius/actions'

class GeniusAuthenticationFrame extends React.Component{

	constructor(props){
		super(props);

		this.state = {
			authorizing: false
		}

		this.handleMessage = this.handleMessage.bind(this);
	}

	componentDidMount(){
		window.addEventListener('message', this.handleMessage, false);
	}

	componentWillUnmount(){
		window.removeEventListener("message", this.handleMessage, false);
	}

	handleMessage(event){
		let data = helpers.toJSON(event.data);

		// Only digest messages relevant to us
		if (data.origin != 'auth_genius'){
			return;
		}

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
			this.props.geniusActions.authorizationGranted(data)
			this.props.geniusActions.getMe();
		}

		// Turn off our authorizing switch
		this.setState({authorizing: false});
	}

	startAuthorization(){

		var self = this
		this.setState({authorizing: true})

		// Open an authentication request window
		var url = this.props.authorization_url+'?action=authorize&scope=me'
		var popup = window.open(url,"popup","height=580,width=350");
		popup.name = "GeniusAuthenticationWindow";

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
				<a className="button button--working">
					Authorizing...
				</a>
			)
		} else if (this.props.authorized){
			return (
				<a className="button button--destructive" onClick={e => this.props.geniusActions.revokeAuthorization()}>Log out</a>
			)
		} else {
			return (
				<a className="button button--primary" onClick={e => this.startAuthorization()}>Log in</a>
			)
		}
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		authorization_url: state.genius.authorization_url,
		authorized: state.genius.authorization,
		authorizing: state.genius.authorizing
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		geniusActions: bindActionCreators(geniusActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(GeniusAuthenticationFrame)

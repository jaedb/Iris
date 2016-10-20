
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { createStore, bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import * as actions from '../services/spotify/actions'

class SpotifyAuthenticationFrame extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			frameUrl: '//jamesbarnsley.co.nz/spotmop.php?action=frame',
			authorizing: false
		}
	}

	componentDidMount(){

		let self = this;

		// listen for incoming messages from the authorization iframe
		// this is triggered when authentication is granted from the popup
		window.addEventListener('message', function(event){
			
			// only allow incoming data from our authorized authenticator proxy
			if( !/^https?:\/\/jamesbarnsley\.co\.nz/.test(event.origin) ) return false;
			
			var data = JSON.parse(event.data);
			self.props.actions.authorizationGranted( data );
			self.props.actions.getMe();

			// and turn off our authorizing switch
			self.setState({
				frameUrl: '//jamesbarnsley.co.nz/spotmop.php?action=frame',
				authorizing: false
			})

		}, false);
	}

	startAuthorization(){
		this.setState({
			frameUrl: '//jamesbarnsley.co.nz/spotmop.php?action=authorize&app='+location.protocol+'//'+window.location.host,
			authorizing: true
		})
	}

	renderMe(){
		if( !this.props.spotify.me ) return null;

		if( this.props.spotify.me.display_name ){
			return <span>Logged in as { this.props.spotify.me.display_name }</span>
		}else{
			return <span>Logged in as { this.props.spotify.me.id }</span>
		}
	}

	renderAuthorizeButton(){
		if( this.state.authorizing ){
			return (
				<button disabled>
					<FontAwesome name="circle-o-notch" spin />
					Authorizing...
				</button>
			);
		}else if( this.props.spotify.authorized ){
			return (
				<div>
					<button onClick={() => this.props.actions.removeAuthorization()}>Log out</button>
					{ this.renderMe() }
				</div>
			);
		}else{
			return (
				<button onClick={() => this.startAuthorization()}>Log in</button>
			);
		}
	}

	renderRefreshButton(){
		if( !this.props.spotify.authorized ) return null;

		if( this.props.spotify.refreshing_token ){
			return (
				<button disabled>
					<FontAwesome name="circle-o-notch" spin />
					Refreshing...
				</button>
			);
		}else{
			return (
				<button onClick={() => this.props.actions.refreshToken()}>Refresh token</button>
			);
		}
	}

	render(){
		return (
			<div>
				{ this.renderAuthorizeButton() }
				{ this.renderRefreshButton() }
				<iframe src={this.state.frameUrl} style={{ display: 'none' }}></iframe>
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
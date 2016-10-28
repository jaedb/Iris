
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { createStore, bindActionCreators } from 'redux'

import FontAwesome from 'react-fontawesome'
import Thumbnail from './Thumbnail'

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

		return (
			<div>
				<Thumbnail circle={true} size="small" images={this.props.spotify.me.images} />
				Logged in as {this.props.spotify.me.display_name ? this.props.spotify.me.display_name : null }
				&nbsp;(<Link to={'/user/'+this.props.spotify.me.id}>{ this.props.spotify.me.id }</Link>)
			</div>
		)
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
		}else if( this.props.spotify.authorized ){
			return (
				<div>
					<button onClick={() => this.props.actions.authorizationRevoked()}>Log out</button>
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
					&nbsp;
					Refreshing...
				</button>
			);
		}else{
			return (
				<button onClick={() => this.props.actions.refreshingToken()}>Refresh token</button>
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
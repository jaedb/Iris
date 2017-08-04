
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link, hashHistory } from 'react-router'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import SpotifyAuthenticationFrame from '../SpotifyAuthenticationFrame'
import ConfirmationButton from '../ConfirmationButton'
import PusherConnectionList from '../PusherConnectionList'
import URISchemesList from '../URISchemesList'
import VersionManager from '../VersionManager'
import Header from '../Header'
import Thumbnail from '../Thumbnail'

import * as uiActions from '../../services/ui/actions'
import * as pusherActions from '../../services/pusher/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class Services extends React.Component{

	constructor(props) {
		super(props)

		this.state = {
			spotify_authentication_provider: this.props.spotify.authentication_provider,
			spotify_country: this.props.spotify.country,
			spotify_locale: this.props.spotify.locale,
			input_in_focus: null
		}
	}

	componentWillReceiveProps(newProps){
		var changed = false
		var state = this.state
		
		if (newProps.spotify.authentication_provider != this.state.spotify_authentication_provider && this.state.input_in_focus != 'spotify_authentication_provider'){
			state.spotify_authentication_provider = newProps.spotify.authentication_provider
			changed = true
		}
		
		if (newProps.spotify.country != this.state.spotify_country && this.state.input_in_focus != 'spotify_country'){
			state.spotify_country = newProps.spotify.country
			changed = true
		}
		
		if (newProps.spotify.locale != this.state.spotify_locale && this.state.input_in_focus != 'spotify_locale'){
			state.spotify_locale = newProps.spotify.locale
			changed = true
		}

		if (changed){
			this.setState(state)
		}
	}

	setSpotifyConfig(state = this.state){
		this.props.spotifyActions.setConfig({
			authentication_provider: state.spotify_authentication_provider, 
			country: state.spotify_country, 
			locale: state.spotify_locale 
		});

		this.setState({input_in_focus: null})
	}

	setProvider(provider){
		let state = this.state
		state.spotify_authentication_provider = provider
		this.setSpotifyConfig(state)
		this.setState(state)
	}

	renderSpotifyUser(){

		var user = null
		if (this.props.spotify.me && this.props.spotify.authorization && this.props.spotify.authentication_provider == 'http_api'){
			user = this.props.spotify.me
		} else if (this.props.spotify.backend_username && this.props.spotify.backend_username){
			if (this.props.core.users && this.props.core.users['spotify:user:'+this.props.spotify.backend_username] !== undefined){
				user = this.props.core.users['spotify:user:'+this.props.spotify.backend_username]
			}
		}

		if (user){
			return (
				<Link className="user" to={global.baseURL+'user/'+user.uri}>
					<Thumbnail circle={true} size="small" images={user.images} />
					<span className="user-name">
						{user.display_name ? user.display_name : user.id}
					</span>
				</Link>
			)
		} else if (this.props.spotify.backend_username){
			return (
				<Link className="user" to={global.baseURL+'user/spotify:user:'+this.props.spotify.backend_username}>
					<Thumbnail circle={true} size="small" />
					<span className="user-name">
						{this.props.spotify.backend_username}
					</span>
				</Link>
			)
		} else {
			return (
				<Link className="user">
					<Thumbnail circle={true} size="small" />
					<span className="user-name">
						Unknown
					</span>
				</Link>
			)
		}
	}

	renderSendAuthorizationButton(){
		if (!this.props.spotify.authorized) return null

		return (
			<button onClick={e => this.props.uiActions.openModal('send_authorization', {}) }>
				Share authentication
			</button>
		)
	}

	render(){
		return (
			<div>
				<h4 className="underline">Spotify (access {this.props.spotify.access})</h4>
				{!this.props.uri_schemes.includes('spotify:') ? <span className="red-text">Mopidy-Spotify not running</span> : null}
				<form>

					<div className="field">
						<div className="name">Country</div>
						<div className="input">
							<input 
								type="text"
								onChange={e => this.setState({ spotify_country: e.target.value })} 
								onFocus={e => this.setState({input_in_focus: 'spotify_country'})} 
								onBlur={e => this.setSpotifyConfig() } 
								value={ this.state.spotify_country } />
						</div>
					</div>
					<div className="field">
						<div className="name">Locale</div>
						<div className="input">
							<input 
								type="text"
								onChange={e => this.setState({ spotify_locale: e.target.value })}
								onFocus={e => this.setState({input_in_focus: 'spotify_locale'})} 
								onBlur={e => this.setSpotifyConfig() } 
								value={this.state.spotify_locale} />
						</div>
					</div>

					<div className="field radio">
						<span className="name">Authentication provider</span>
						<div className="input">
							<label>
								<input 
									type="radio"
									name="spotify_authentication_provider"
									value="backend"
									checked={this.props.spotify.authentication_provider == 'backend'}
									onChange={e => this.setProvider(e.target.value)}
								/>
								<span className="label">Mopidy-Spotify</span>
							</label>
							<label>
								<input 
									type="radio"
									name="spotify_authentication_provider"
									value="http_api"
									checked={this.props.spotify.authentication_provider == 'http_api' }
									onChange={e => this.setProvider(e.target.value)}
								/>
								<span className="label">HTTP API</span>
							</label>
						</div>
					</div>
					{this.props.spotify.authentication_provider == 'http_api' ? <div className="field">
						<div className="name">Authentication</div>
						<div className="input">
							<SpotifyAuthenticationFrame />
							{ this.renderSendAuthorizationButton() }
						</div>
					</div> : null}
					<div className="field current-user">
						<div className="name">Current user</div>
						<div className="input">
							<div className="text">
								{ this.renderSpotifyUser() }
							</div>
						</div>
					</div>
				</form>
			</div>
		);
	}
}


/**
 * Export our component
 *
 * We also integrate our global store, using connect()
 **/

const mapStateToProps = (state, ownProps) => {
	return {
		uri_schemes: (state.mopidy.uri_schemes ? state.mopidy.uri_schemes : []),
		core: state.core,
		spotify: state.spotify
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Services)
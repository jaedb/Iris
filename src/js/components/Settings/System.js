
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link, hashHistory } from 'react-router'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import SpotifyAuthenticationFrame from '../SpotifyAuthenticationFrame'
import Thumbnail from '../Thumbnail'

import * as coreActions from '../../services/core/actions'
import * as uiActions from '../../services/ui/actions'
import * as pusherActions from '../../services/pusher/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class System extends React.Component{

	constructor(props) {
		super(props);
		this.state = {
			country: this.props.core.country,
			locale: this.props.core.locale,
			mopidy_host: this.props.mopidy.host,
			mopidy_port: this.props.mopidy.port,
			pusher_username: this.props.pusher.username,
			input_in_focus: null
		}
	}

	componentWillReceiveProps(newProps){
		var changed = false
		var state = this.state
		
		if (newProps.pusher.username != this.state.pusher_username && this.state.input_in_focus != 'pusher_username'){
			state.pusher_username = newProps.pusher.username
			changed = true
		}

		if (changed){
			this.setState(state)
		}
	}

	resetAllSettings(){
		localStorage.clear();
		window.location = '#'
		window.location.reload(true)
		return false;
	}

	setMopidyConfig(e){
		this.setState({input_in_focus: null})
		e.preventDefault();
		this.props.mopidyActions.setConfig({ host: this.state.mopidy_host, port: this.state.mopidy_port });
		window.location.reload(true);
		return false;
	}

	handleBlur(name, value){
		this.setState({input_in_focus: null})
		var data = {}
		data[name] = value
		this.props.coreActions.set(data)
	}

	handleUsernameChange(username){
		this.setState({pusher_username: username.replace(/\W/g, '')})
	}

	handleUsernameBlur(e){
		this.setState({input_in_focus: null})
		this.props.pusherActions.setUsername(this.state.pusher_username)
	}

	renderApplyButton(){
		if (this.props.mopidy.host == this.state.mopidy_host && this.props.mopidy.port == this.state.mopidy_port) return null

		return (
			<div className="field">
				<div className="name"></div>
				<div className="input">
					<button type="submit" className="secondary">Apply and reload</button>
				</div>
			</div>
		)
	}

	renderStatus(server){
		if (this.props[server].connecting){
			return <FontAwesome name="plug" className="grey-text pulse" />
		} else if (this.props[server].connected){
			return <FontAwesome name="check" className="green-text" />
		} else {
			return <FontAwesome name="exclamation-triangle" className="red-text" />
		}
	}

	renderSpotifyUser(){

		var user = null
		if (this.props.spotify.me && this.props.spotify.authorization){
			user = this.props.spotify.me
		} else if (this.props.spotify.backend_username){
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
		if (!this.props.spotify.authorization) return null

		return (
			<button onClick={e => this.props.uiActions.openModal('send_authorization', {}) }>
				Share authentication
			</button>
		)
	}

	render(){
		return (
			<div>

				<h4 className="underline">Servers</h4>

				<div className="field readonly">
					<div className="name">Status</div>
					<div className="input">
						<div className="text">
							{this.renderStatus('mopidy')}&nbsp; Mopidy &nbsp;&nbsp;
							{this.renderStatus('pusher')}&nbsp; Pusher
						</div>
					</div>
				</div>

				<div className="field">
					<div className="name">Username</div>
					<div className="input">
						<input 
							type="text"
							onChange={e => this.handleUsernameChange(e.target.value)} 
							onFocus={e => this.setState({input_in_focus: 'pusher_username'})}
							onBlur={e => this.handleUsernameBlur(e)}
							value={this.state.pusher_username } />
					</div>
				</div>

				<form onSubmit={(e) => this.setMopidyConfig(e)}>
					<div className="field">
						<div className="name">Host</div>
						<div className="input">
							<input 
								type="text"
								onChange={ e => this.setState({mopidy_host: e.target.value})} 
								onFocus={e => this.setState({input_in_focus: 'mopidy_host'})}
								onBlur={e => this.setState({input_in_focus: null})} 
								value={ this.state.mopidy_host } />
						</div>
					</div>
					<div className="field">
						<div className="name">Port</div>
						<div className="input">
							<input 
								type="text"
								onChange={ e => this.setState({mopidy_port: e.target.value})} 
								onFocus={e => this.setState({input_in_focus: 'mopidy_port'})} 
								onBlur={e => this.setState({input_in_focus: null})} 
								value={ this.state.mopidy_port } />
						</div>
					</div>
					{this.renderApplyButton()}
				</form>

				<h4 className="underline">Localization</h4>

				<div className="field">
					<div className="name">Country</div>
					<div className="input">
						<input 
							type="text"
							onChange={e => this.setState({country: e.target.value})} 
							onFocus={e => this.setState({input_in_focus: 'country'})} 
							onBlur={e => this.handleBlur('country',e.target.value)} 
							value={ this.state.country } />
					</div>
				</div>
				<div className="field">
					<div className="name">Locale</div>
					<div className="input">
						<input 
							type="text"
							onChange={e => this.setState({locale: e.target.value})}
							onFocus={e => this.setState({input_in_focus: 'locale'})} 
							onBlur={e => this.handleBlur('locale',e.target.value)} 
							value={this.state.locale} />
					</div>
				</div>

				<h4 className="underline">Spotify</h4>

				<div className="field readonly">
					<div className="name">Status</div>
					<div className="input">
						<div className="text">
							{!this.props.mopidy.uri_schemes || !this.props.mopidy.uri_schemes.includes('spotify:') ? <div className="red-text"><FontAwesome name="exclamation-triangle" />&nbsp; Mopidy-Spotify not available</div> : null}

							{this.props.spotify.authorization ? <span><span className="green-text"><FontAwesome name="check" /> Authorized</span>&nbsp; All Spotify functionality available</span> : <span><span className="orange-text"><FontAwesome name="lock" /> Limited access</span>&nbsp; Authorize Iris for full functionality</span>}
						</div>
					</div>
				</div>
				<div className="field">
					<div className="name">Authorization</div>
					<div className="input">
						<SpotifyAuthenticationFrame />
						{ this.renderSendAuthorizationButton() }
					</div>
				</div>
				<div className="field current-user">
					<div className="name">Current user</div>
					<div className="input">
						<div className="text">
							{ this.renderSpotifyUser() }
						</div>
					</div>
				</div>

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
		core: state.core,
		ui: state.ui,
		mopidy: state.mopidy,
		pusher: state.pusher,
		spotify: state.spotify
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(System)
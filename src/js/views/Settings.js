
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { hashHistory, Link } from 'react-router'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import SpotifyAuthenticationFrame from '../components/SpotifyAuthenticationFrame'
import ConfirmationButton from '../components/ConfirmationButton'
import PusherConnectionList from '../components/PusherConnectionList'
import URISchemesList from '../components/URISchemesList'
import VersionManager from '../components/VersionManager'
import Header from '../components/Header'
import Parallax from '../components/Parallax'
import Icon from '../components/Icon'
import Thumbnail from '../components/Thumbnail'

import * as coreActions from '../services/core/actions'
import * as uiActions from '../services/ui/actions'
import * as pusherActions from '../services/pusher/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Settings extends React.Component {

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
		
		if (newProps.spotify.country != this.state.spotify_country && this.state.input_in_focus != 'spotify_country'){
			state.spotify_country = newProps.spotify.country
			changed = true
		}
		
		if (newProps.spotify.locale != this.state.spotify_locale && this.state.input_in_focus != 'spotify_locale'){
			state.spotify_locale = newProps.spotify.locale
			changed = true
		}
		
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

	renderServiceStatus(service){

		let colour = 'red'
		let icon = 'close'
		let name = service.charAt(0).toUpperCase() + service.slice(1).toLowerCase()
		let text = 'Disconnected'
		let tooltip = null

		service = this.props[service]

		if (service.connecting){
			icon = 'plug'
			colour = 'grey'
			text = 'Connecting'
		} else if (name == 'Spotify' && (!this.props.mopidy.uri_schemes || !this.props.mopidy.uri_schemes.includes('spotify:'))){
			icon = 'exclamation-triangle'
			colour = 'red'
			text = 'Not installed'
			tooltip = 'Mopidy-Spotify is not installed or enabled'
		} else if (service.connected && name == 'Spotify' && !service.authorization){
			icon = 'lock'
			colour = 'orange'
			text = 'Limited access'
			tooltip = 'Authorize Iris for full Spotify functionality'
		} else if (service.connected){
			icon = 'check'
			colour = 'green'
			text = 'Connected'
		}

		return (
			<div className={"service"+(tooltip ? ' has-tooltip large-tooltip' : '')}>
				<h4 className="title">
					{name}
				</h4>
				<div className={colour+'-text icon'}>
					<FontAwesome name={icon} />
				</div>
				<div className={"status "+colour+'-text'}>					
					{text}
				</div>
				{tooltip ? <span className="tooltip">{tooltip}</span> : null}
			</div>
		)
	}

	render(){

		var options = (
			<span>
				<button className="no-hover" onClick={e => hashHistory.push(global.baseURL+'settings/debug')}>
					<FontAwesome name="flask" />&nbsp;
					Debug
					{this.props.ui && this.props.ui.test_mode ? <span className="flag warning">Test mode</span> : null}
				</button>
				<a className="no-hover button" href="https://github.com/jaedb/Iris/wiki" target="_blank">
					<FontAwesome name="question" />&nbsp;
					Help
				</a>
			</span>
		)

		return (
			<div className="view settings-view">
				<Header className="overlay" icon="cog" title="Settings" options={options} uiActions={this.props.uiActions} />

				<div className="intro">
					<div className="liner">
						<Parallax image="/iris/assets/backgrounds/settings.jpg" />
					</div>
				</div>

				<section className="content-wrapper">

					<div className="services">
						{this.renderServiceStatus('mopidy')}
						{this.renderServiceStatus('pusher')}
						{this.renderServiceStatus('spotify')}
					</div>

					<h4 className="underline">System</h4>

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

					<h4 className="underline">Advanced</h4>

					<div className="field checkbox">
						<div className="name">UI behavior</div>
						<div className="input">
							<label>
								<input 
									type="checkbox"
									name="log_actions"
									checked={ this.props.ui.clear_tracklist_on_play }
									onChange={ e => this.props.uiActions.set({ clear_tracklist_on_play: !this.props.ui.clear_tracklist_on_play })} />
								<span className="label">Clear tracklist on play of URI(s)</span>
							</label>
						</div>
					</div>

					<div className="field pusher-connections">
						<div className="name">Connections</div>
						<div className="input">
							<div className="text">
			        			<PusherConnectionList />
			        		</div>
			        	</div>
			        </div>
					
					<div className="field">
						<div className="name">Extensions</div>
						<div className="input">
				        	<div className="text">
				        		<URISchemesList />
				        	</div>
				        </div>
			        </div>
					
					<div className="field">
						<div className="name">Version</div>
						<div className="input">
				        	<VersionManager />
				        </div>
			        </div>
					
					<div className="field">
						<div className="name">Reset</div>
						<div className="input">
					        <ConfirmationButton className="destructive" content="Reset all settings" confirmingContent="Are you sure?" onConfirm={() => this.resetAllSettings()} />
					        <button onClick={e => this.props.spotifyActions.refreshingToken()}>Refresh Spotify token</button>
				        </div>
			        </div>

					<h4 className="underline">About</h4>

					<div className="field">
						<div>
							<em><a href="https://github.com/jaedb/Iris" target="_blank">Iris</a></em> is an open-source project by <a href="https://github.com/jaedb" target="_blank">James Barnsley</a>. It is provided free and with absolutely no warranty. If you paid someone for this software, please let me know.
								<br />
								<br />
								Google Analytics is used to help trace issues and provide valuable insight into how we can continue to make improvements.
								<br />
						</div>
						<br /><br />
						<div>
					        <a className="button" href="https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=james%40barnsley%2enz&lc=NZ&item_name=James%20Barnsley&currency_code=USD&bn=PP%2dDonationsBF%3abtn_donate_LG%2egif%3aNonHosted" target="_blank">
					        	<FontAwesome name="paypal" />&nbsp;Donate
					        </a>
					        &nbsp;&nbsp;
					        <a className="button" href="https://github.com/jaedb/Iris" target="_blank">
					        	<FontAwesome name="github" />&nbsp;GitHub
					        </a>
					        &nbsp;&nbsp;
					        <a rel="license" href="http://creativecommons.org/licenses/by-nc/4.0/" target="_blank" style={{display: 'inline-block', verticalAlign: 'middle'}}><img alt="Creative Commons License" src="https://i.creativecommons.org/l/by-nc/4.0/88x31.png" /></a>
						</div>
			        </div>

		        </section>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return state;
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

export default connect(mapStateToProps, mapDispatchToProps)(Settings)
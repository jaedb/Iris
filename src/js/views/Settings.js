
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
import Icon from '../components/Icon'
import Thumbnail from '../components/Thumbnail'

import * as uiActions from '../services/ui/actions'
import * as pusherActions from '../services/pusher/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Settings extends React.Component{

	constructor(props) {
		super(props);
		this.state = {
			mopidy_host: this.props.mopidy.host,
			mopidy_port: this.props.mopidy.port,
			spotify_country: this.props.spotify.country,
			spotify_locale: this.props.spotify.locale,
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

	setSpotifyConfig(e){
		this.setState({input_in_focus: null})
		e.preventDefault();
		this.props.spotifyActions.setConfig({ country: this.state.spotify_country, locale: this.state.spotify_locale });
		return false;
	}

	handleUsernameChange(username){
		this.setState({pusher_username: username.replace(/\W/g, '')})
	}

	handleUsernameBlur(e){
		this.setState({input_in_focus: null})
		this.props.pusherActions.setUsername(this.state.pusher_username)
	}

	renderSpotifyUser(){

		var user = null
		if (this.props.spotify.me && this.props.spotify.authorized){
			user = this.props.spotify.me
		} else if (this.props.ui.config && this.props.ui.config.spotify_username){
			if (this.props.ui.users && typeof(this.props.ui.users['spotify:user:'+this.props.ui.config.spotify_username]) !== 'undefined'){
				user = this.props.ui.users['spotify:user:'+this.props.ui.config.spotify_username]
			}
		}

		if (user){
			return (
				<Link className="user" to={global.baseURL+'user/'+user.uri}>
					<Thumbnail circle={true} size="small" images={user.images} />
					<span className="user-name">
						{user.display_name ? user.display_name : user.id}
						{!this.props.spotify.authorized ? <span className="grey-text">&nbsp;(limited access)</span> : null}
					</span>
				</Link>
			)
		} else {
			return (
				<Link className="user">
					<Thumbnail circle={true} size="small" />
					<span className="user-name">
						Default user
						<span className="grey-text">&nbsp;(limited access)</span>
					</span>
				</Link>
			)
		}
	}

	renderSendAuthorizationButton(){
		if( !this.props.spotify.authorized ) return null

		return (
			<button onClick={e => this.props.uiActions.openModal('send_authorization', {}) }>
				Share authentication
			</button>
		)
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

	serviceStatus(service){

		var icon = null
		var status = 'Unknown'

		if (this.props[service].enabled !== undefined && !this.props[service].enabled){
			icon = <span className="icon"><FontAwesome fixedWidth name="power-off" /></span>
			status = 'disabled'
		} else if (this.props[service].connecting){
			icon = <span className="icon"><FontAwesome fixedWidth name="plug" /></span>
			status = 'connecting'
		} else if (this.props[service].connected){
			icon = <span className="icon"><FontAwesome fixedWidth name="check" /></span>
			status = 'connected'
		} else {
			icon = <span className="icon disconnected"><FontAwesome fixedWidth name="close" /></span>
			status = 'disconnected'
		}

		return (
			<div className={'service '+status}>
				<span className="content">
					<h4 className="title">{service}</h4>
					<div className="status">{icon}&nbsp; {status}</div>
				</span>
			</div>
		)
	}

	render(){

		var options = (
			<span>
				<button className="no-hover" onClick={e => hashHistory.push(global.baseURL+'settings/debug')}>
					<FontAwesome name="flask" />&nbsp;
					Debug
				</button>
				<a className="no-hover button" href="https://github.com/jaedb/Iris/wiki" target="_blank">
					<FontAwesome name="question" />&nbsp;
					Help
				</a>
			</span>
		)

		return (
			<div className="view settings-view">
				<Header icon="cog" title="Settings" options={options} uiActions={this.props.uiActions} />

				<section className="content-wrapper">

					<div className="services">					
						{this.serviceStatus('mopidy')}
						{this.serviceStatus('pusher')}
						{this.serviceStatus('spotify')}
					</div>

					<h4 className="underline">System</h4>
					<form onSubmit={(e) => this.setMopidyConfig(e)}>
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
						<div className="field">
							<div className="name">Host</div>
							<div className="input">
								<input 
									type="text"
									onChange={ e => this.setState({ mopidy_host: e.target.value })} 
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
									onChange={ e => this.setState({ mopidy_port: e.target.value })} 
									onFocus={e => this.setState({input_in_focus: 'mopidy_port'})} 
									onBlur={e => this.setState({input_in_focus: null})} 
									value={ this.state.mopidy_port } />
							</div>
						</div>
						{this.renderApplyButton()}
					</form>

					<h4 className="underline">Spotify</h4>
					<form>
						<div className="field checkbox">
							<div className="input">
								<label>
									<input 
										type="checkbox"
										name="spotify_enabled"
										checked={ this.props.spotify.enabled }
										onChange={e => this.props.spotifyActions.setConfig({enabled: !this.props.spotify.enabled})} 
									/>
									<span className="label">Enabled</span>
								</label>
							</div>
						</div>
						<div className="field">
							<div className="name">Country</div>
							<div className="input">
								<input 
									type="text"
									onChange={e => this.setState({ spotify_country: e.target.value })} 
									onFocus={e => this.setState({input_in_focus: 'spotify_country'})} 
									onBlur={e => this.setSpotifyConfig(e) } 
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
									onBlur={e => this.setSpotifyConfig(e) } 
									value={this.state.spotify_locale} />
							</div>
						</div>
					</form>
					<div className="field current-user">
						<div className="name">Current user</div>
						<div className="input">
							<div className="text">
								{ this.renderSpotifyUser() }
							</div>
						</div>
					</div>
					<div className="field">
						<div className="name">Authentication</div>
						<div className="input">
							<SpotifyAuthenticationFrame />
							{ this.renderSendAuthorizationButton() }
						</div>
					</div>

					<h4 className="underline">Advanced</h4>

					<div className="field checkbox">
						<div className="name">Customise behavior</div>
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
							<span className="text">
			        			<PusherConnectionList />
			        		</span>
			        	</div>
			        </div>
					
					<div className="field">
						<div className="name">Backends</div>
						<div className="input">
							<span className="text">
				        		<URISchemesList />
			        		</span>
			        	</div>
			        </div>
					
					<div className="field">
						<div className="name">System</div>
						<div className="input">
					        <ConfirmationButton className="destructive" content="Reset all settings" confirmingContent="Are you sure?" onConfirm={() => this.resetAllSettings()} />
				        	<VersionManager />
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


/**
 * Export our component
 *
 * We also integrate our global store, using connect()
 **/

const mapStateToProps = (state, ownProps) => {
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Settings)
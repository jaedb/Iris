
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { hashHistory, Link } from 'react-router'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import SpotifyAuthenticationFrame from '../components/SpotifyAuthenticationFrame'
import LastfmAuthenticationFrame from '../components/LastfmAuthenticationFrame'
import ConfirmationButton from '../components/ConfirmationButton'
import PusherConnectionList from '../components/PusherConnectionList'
import URISchemesList from '../components/URISchemesList'
import VersionManager from '../components/VersionManager'
import Header from '../components/Header'
import Parallax from '../components/Parallax'
import Icon from '../components/Icon'
import Thumbnail from '../components/Thumbnail'
import URILink from '../components/URILink'

import * as coreActions from '../services/core/actions'
import * as uiActions from '../services/ui/actions'
import * as pusherActions from '../services/pusher/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as lastfmActions from '../services/lastfm/actions'
import * as spotifyActions from '../services/spotify/actions'

class Settings extends React.Component {

	constructor(props){
		super(props);
		this.state = {
			country: this.props.core.country,
			locale: this.props.core.locale,
			mopidy_host: this.props.mopidy.host,
			mopidy_port: this.props.mopidy.port,
			mopidy_ssl: this.props.mopidy.ssl,
			pusher_username: this.props.pusher.username,
			input_in_focus: null
		}
	}

	componentDidMount(){
		if (this.props.lastfm.session && this.props.core.users["lastfm:user:"+this.props.lastfm.session.name] === undefined){
			this.props.lastfmActions.getMe();
		}
	}

	componentWillReceiveProps(newProps){
		var changed = false
		var state = this.state
		
		if (newProps.core.country != this.state.country && this.state.input_in_focus != 'country'){
			state.country = newProps.core.country
			changed = true
		}
		
		if (newProps.core.locale != this.state.locale && this.state.input_in_focus != 'locale'){
			state.locale = newProps.core.locale
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

	setConfig(e){
		this.setState({input_in_focus: null});
		e.preventDefault();
		
		this.props.mopidyActions.setConfig({
			host: this.state.mopidy_host,
			port: this.state.mopidy_port,
			ssl: this.state.mopidy_ssl
		});

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
		if (this.props.mopidy.host == this.state.mopidy_host && 
			this.props.mopidy.port == this.state.mopidy_port && 
			this.props.mopidy.ssl == this.state.mopidy_ssl){
				return null;
		}

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
		var user = this.props.spotify.me;

		if (user){
			return (
				<URILink className="user" type="user" uri={user.uri}>
					<Thumbnail circle={true} size="small" images={user.images} />
					<span className="user-name">
						{user.display_name ? user.display_name : user.id}
						{!this.props.spotify.authorization ? <span className="grey-text">&nbsp;&nbsp;(Limited access)</span> : null}
					</span>
				</URILink>
			)
		} else {
			return (
				<URILink className="user">
					<Thumbnail circle={true} size="small" />
					<span className="user-name">
						Unknown
					</span>
				</URILink>
			)
		}
	}

	renderLastfmUser(){
		var user = this.props.core.users["lastfm:user:"+this.props.lastfm.session.name];

		if (user){
			return (
				<span className="user">
					<Thumbnail circle={true} size="small" images={user.image} />
					<span className="user-name">
						{user.realname ? user.realname : user.name}
					</span>
				</span>
			)
		} else {
			return (
				<span className="user">
					<Thumbnail circle={true} size="small" />
					<span className="user-name">
						Unknown
					</span>
				</span>
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

	renderServerStatus(){
		var colour = 'grey';
		var icon = 'question-circle';
		var status = 'Unknown';

		if (this.props.mopidy.connecting || this.props.pusher.connecting){
			icon = 'plug';
			status = 'Connecting...'
		} else if (!this.props.mopidy.connected || !this.props.pusher.connected){
			colour = 'red';
			icon = 'close';
			status = 'Disconnected';
		} else if (this.props.mopidy.connected && this.props.pusher.connected){
			colour = 'green';
			icon = 'check';
			status = 'Connected';
		}

		return (
			<span className={colour+'-text'}>
				<FontAwesome name={icon} />&nbsp; {status}
			</span>
		);
	}

	renderSpotifyStatus(){
		var colour = 'grey';
		var icon = 'question-circle';
		var status = 'Unknown';

		if (this.props.spotify.connecting){
			icon = 'plug';
			status = 'Connecting...'
		} else if (!this.props.spotify.connected){
			colour = 'red';
			icon = 'close';
			status = 'Disconnected';
		} else if (this.props.mopidy.connected){
			colour = 'green';
			icon = 'check';
			status = 'Connected';
		}

		if (!this.props.mopidy.uri_schemes || !this.props.mopidy.uri_schemes.includes('spotify:')){
			colour = 'orange';
			status += ' (Mopidy-Spotify extension not installed/enabled!)';
		}

		return (
			<span className={colour+'-text'}>
				<FontAwesome name={icon} />&nbsp; {status}
			</span>
		);
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
						<Parallax image="assets/backgrounds/settings.jpg" />
					</div>
				</div>

				<section className="content-wrapper">

					<h4 className="underline">Server</h4>

					<div className="field">
						<div className="name">Status</div>
						<div className="input">
							<div className="text">
								{this.renderServerStatus()}
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
							<div className="description">
								A non-unique string used to identify this client (no special characters)
							</div>
						</div>
					</div>

					<form onSubmit={(e) => this.setConfig(e)}>
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
						<div className="field checkbox">
							<div className="name">Encryption</div>
							<div className="input">
								<label>
									<input 
										type="checkbox"
										name="ssl"
										checked={this.state.mopidy_ssl}
										onChange={e => this.setState({mopidy_ssl: !this.state.mopidy_ssl})} />
									<span className="label has-tooltip">
										Enable SSL
										<span className="tooltip">Requires SSL proxy</span>
									</span>
								</label>
							</div>
						</div>
						{this.renderApplyButton()}
					</form>					

					<h4 className="underline">Streaming</h4>

					<div className="field checkbox">
						<div className="name">Enable</div>
						<div className="input">
							<label>
								<input 
									type="checkbox"
									name="ssl"
									checked={this.props.core.http_streaming_enabled}
									onChange={e => this.props.coreActions.set({http_streaming_enabled: !this.props.core.http_streaming_enabled})} />
								<span className="label has-tooltip">
									Enable HTTP streaming
									<span className="tooltip">Requires streaming service like Icecast2</span>
								</span>
							</label>
						</div>
					</div>
					<div className="field">
						<div className="name">Stream location</div>
						<div className="input">
							<input 
								type="text"
								onChange={e => this.props.coreActions.set({http_streaming_url: e.target.value})}
								value={this.props.core.http_streaming_url} />
							<div className="description">
								The full URL to your stream endpoint
							</div>
						</div>
					</div>

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
							<div className="description">
								An <a href="http://en.wikipedia.org/wiki/ISO_3166-1_alpha-2" target="_blank">ISO 3166-1 alpha-2</a> country code (eg <em>NZ</em>)
							</div>
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
							<div className="description">
								Lowercase <a href="http://en.wikipedia.org/wiki/ISO_639" target="_blank">ISO 639 language code</a> and an uppercase <a href="http://en.wikipedia.org/wiki/ISO_3166-1_alpha-2" target="_blank">ISO 3166-1 alpha-2 country code</a>, joined by an underscore (eg <em>en_NZ</em>)
							</div>
						</div>
					</div>

					<h4 className="underline">Spotify</h4>

					<div className="field">
						<div className="name">Status</div>
						<div className="input">
							<div className="text">
								{this.renderSpotifyStatus()}
							</div>
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

					<div className="field">
						<div className="name">Authorization</div>
						<div className="input">
							<SpotifyAuthenticationFrame />
							{ this.renderSendAuthorizationButton() }
							{this.props.spotify.refreshing_token ? <button className="working">Refreshing...</button> : <button onClick={e => this.props.spotifyActions.refreshingToken()}>Force token refresh</button>}
						</div>
					</div>

					<h4 className="underline">LastFM</h4>

					{this.props.lastfm.session ? <div className="field current-user">
						<div className="name">Current user</div>
						<div className="input">
							<div className="text">
								{ this.renderLastfmUser() }
							</div>
						</div>
					</div> : null}

					<div className="field">
						<div className="name">Authorization</div>
						<div className="input">
							<LastfmAuthenticationFrame />
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
								<span className="label has-tooltip">
									Clear tracklist on play of URI(s)
									<span className="tooltip">Playing one or more URIs will clear the current play queue first</span>
								</span>
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
				        </div>
			        </div>

					<h4 className="underline">About</h4>

					<div className="field">
						<div>
							<em><a href="https://github.com/jaedb/Iris" target="_blank">Iris</a></em> is an open-source project by <a href="https://github.com/jaedb" target="_blank">James Barnsley</a>. It is provided free and with absolutely no warranty. If you paid someone for this software, please let me know.
								<br />
								<br />
								Google Analytics is used to collect usage data and errors to help trace issues and provide valuable insight into how we can continue to make improvements. Personal information is anonymized prior to collection. For more information, see <a href="https://github.com/jaedb/Iris/wiki/Terms-of-use" target="_blank">terms and conditions</a>.
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
					        <a className="button" href="http://creativecommons.org/licenses/by-nc/4.0/" target="_blank"><FontAwesome name="creative-commons" />&nbsp;Licence</a>
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
		lastfmActions: bindActionCreators(lastfmActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Settings)
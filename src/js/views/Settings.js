
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
			pusher_username: this.props.pusher.username,
			spotify_country: this.props.spotify.country,
			spotify_locale: this.props.spotify.locale
		};
	}

	resetAllSettings(){
		localStorage.clear();
		window.location = '#'
		window.location.reload(true)
		return false;
	}

	setMopidyConfig(e){
		e.preventDefault();
		this.props.mopidyActions.setConfig({ host: this.state.mopidy_host, port: this.state.mopidy_port });
		window.location.reload(true);
		return false;
	}

	setSpotifyConfig(e){
		e.preventDefault();
		this.props.spotifyActions.setConfig({ country: this.state.spotify_country, locale: this.state.spotify_locale });
		return false;
	}

	handleUsernameChange(username){
		this.setState({pusher_username: username.replace(/\W/g, '')})
	}

	renderConnectionStatus(service){
		if (service == 'Spotify' && !this.props[service.toLowerCase()].authorized){			
			return (
				<span className="red-text connection-status">
					<FontAwesome name="exclamation-triangle" />
					&nbsp;
					Not connected
				</span>
			)
		} else if (this.props[service.toLowerCase()].connected){
			return (
				<span className="green-text connection-status">
					<FontAwesome name="check" />
					&nbsp;
					{service}
				</span>
			)
		} else if (this.props[service.toLowerCase()].connecting){			
			return (
				<span className="grey-text connection-status">
					<FontAwesome name="circle-o-notch" spin />
					&nbsp;
					Connecting
				</span>
			)
		} else {			
			return (
				<span className="red-text connection-status">
					<FontAwesome name="exclamation-triangle" />
					&nbsp;
					Not connected
				</span>
			)
		}
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
		if (this.props[service].connecting){
			return <span className="grey-text">Connecting</span>
		} else if (service == 'spotify' && !this.props[service].authorized){
			return <span className="orange-text">Not authorized</span>
		} else if (this.props[service].connected){
			return <span className="green-text">Connected</span>
		} else {
			return <span className="red-text">Disconnected</span>
		}
	}

	render(){

		var options = (
			<span>
				<button onClick={e => hashHistory.push(global.baseURL+'settings/debug')}>
					<FontAwesome name="flask" />&nbsp;
					Debug
				</button>
				<a className="button" href="https://github.com/jaedb/Iris/wiki" target="_blank">
					<FontAwesome name="question" />&nbsp;
					Help
				</a>
			</span>
		)

		return (
			<div className="view settings-view">
				<Header icon="cog" title="Settings" options={options} uiActions={this.props.uiActions} />

				<section>

					<div className="status">
					
						<div className="item">
							<Icon name="server" />
							<h4>Mopidy</h4>
							{this.serviceStatus('mopidy')}
						</div>
					
						<div className="item">
							<Icon name="connection" />
							<h4>Pusher</h4>
							{this.serviceStatus('pusher')}
						</div>

						<div className="item">
							<Icon name="spotify" />
							<h4>Spotify</h4>
							{this.serviceStatus('spotify')}
						</div>

					</div>

					<h4 className="underline">System</h4>
					<form onSubmit={(e) => this.setMopidyConfig(e)}>
						<div className="field">
							<div className="name">Username</div>
							<div className="input">
								<input 
									type="text"
									onChange={e => this.handleUsernameChange(e.target.value)} 
									onBlur={ e => this.props.pusherActions.setUsername(this.state.pusher_username) } 
									value={ this.state.pusher_username } />
							</div>
						</div>
						<div className="field">
							<div className="name">Host</div>
							<div className="input">
								<input 
									type="text"
									onChange={ e => this.setState({ mopidy_host: e.target.value })} 
									value={ this.state.mopidy_host } />
							</div>
						</div>
						<div className="field">
							<div className="name">Port</div>
							<div className="input">
								<input 
									type="text"
									onChange={ e => this.setState({ mopidy_port: e.target.value })} 
									value={ this.state.mopidy_port } />
							</div>
						</div>
						{this.renderApplyButton()}
					</form>

					<h4 className="underline">Spotify</h4>
					<form>
						<div className="field">
							<div className="name">Country</div>
							<div className="input">
								<input 
									type="text"
									onChange={ e => this.setState({ spotify_country: e.target.value })} 
									onBlur={ e => this.setSpotifyConfig(e) } 
									value={ this.state.spotify_country } />
							</div>
						</div>
						<div className="field">
							<div className="name">Locale</div>
							<div className="input">
								<input 
									type="text"
									onChange={ e => this.setState({ spotify_locale: e.target.value })} 
									onBlur={ e => this.setSpotifyConfig(e) } 
									value={ this.state.spotify_locale } />
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
					        <a className="button" href="https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=james%40barnsley%2enz&lc=NZ&item_name=James%20Barnsley&currency_code=NZD&bn=PP%2dDonationsBF%3abtn_donate_LG%2egif%3aNonHosted" target="_blank">
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
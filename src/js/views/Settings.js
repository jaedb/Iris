
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { hashHistory, Link } from 'react-router'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import ConfirmationButton from '../components/Fields/ConfirmationButton'
import PusherConnectionList from '../components/PusherConnectionList'
import Header from '../components/Header'
import Parallax from '../components/Parallax'
import Icon from '../components/Icon'
import Thumbnail from '../components/Thumbnail'
import URILink from '../components/URILink'
import Services from '../components/Services'

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
			mopidy_host: this.props.mopidy.host,
			mopidy_port: this.props.mopidy.port,
			mopidy_ssl: this.props.mopidy.ssl,
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

		
		if (this.props.mopidy.upgrading){
			var upgrade_button = (
				<button className="alternative working">
					Upgrading...
				</button>
			);
		} else if (this.props.pusher.version.upgrade_available){
			var upgrade_button = <button className="alternative" onClick={e => this.props.pusherActions.upgrade()}>Upgrade to { this.props.pusher.version.latest }</button>;
		} else {
			var upgrade_button = null;
		}

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

					<h4 className="underline">Services</h4>
					<Services active={this.props.params.sub_view} />

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
						<div className="name">Version</div>
						<div className="input">
				        	<span className="text">
				        		{this.props.pusher.version.current} installed {this.props.pusher.version.upgrade_available ? <span className="flag blue">Upgrade available</span> : <span className="flag dark"><FontAwesome name="check" className="green-text" />&nbsp; Up-to-date</span>}
				        	</span>
				        </div>
			        </div>
					
					<div className="field">
						{upgrade_button}
				        <button className={"destructive"+(this.props.mopidy.restarting ? ' working' : '')} onClick={e => this.props.pusherActions.restart()}>{this.props.mopidy.restarting ? 'Restarting...' : 'Restart server'}</button>
				        <ConfirmationButton className="destructive" content="Reset all settings" confirmingContent="Are you sure?" onConfirm={() => this.resetAllSettings()} />
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
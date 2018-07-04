
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { hashHistory, Link } from 'react-router'
import { bindActionCreators } from 'redux'

import ConfirmationButton from '../components/Fields/ConfirmationButton'
import PusherConnectionList from '../components/PusherConnectionList'
import SourcesPriority from '../components/Fields/SourcesPriority'
import Header from '../components/Header'
import Parallax from '../components/Parallax'
import Icon from '../components/Icon'
import Thumbnail from '../components/Thumbnail'
import URILink from '../components/URILink'
import Services from '../components/Services'

import * as helpers from '../helpers'
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
			mopidy_library_artists_uri: this.props.mopidy.library_artists_uri,
			mopidy_library_albums_uri: this.props.mopidy.library_albums_uri,
			pusher_username: this.props.pusher.username,
			input_in_focus: null
		}
	}

	componentDidMount(){
		this.props.uiActions.setWindowTitle("Settings");
	}

	componentWillReceiveProps(nextProps){
		var changed = false
		var state = this.state
		
		if (nextProps.pusher.username != this.state.pusher_username && this.state.input_in_focus != 'pusher_username'){
			state.pusher_username = nextProps.pusher.username
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
		
		this.props.mopidyActions.set({
			host: this.state.mopidy_host,
			port: this.state.mopidy_port,
			ssl: this.state.mopidy_ssl
		});

		window.location.reload(true);
		return false;
	}

	handleBlur(service, name, value){
		this.setState({input_in_focus: null});
		var data = {};
		data[name] = value;
		this.props[service+'Actions'].set(data);

		// Any per-field actions
		switch (name){
			case 'library_albums_uri':
				this.props.mopidyActions.clearLibraryAlbums();
				break;
			case 'library_artists_uri':
				this.props.mopidyActions.clearLibraryArtists();
				break;
		}
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
		var icon = 'help';
		var status = 'Unknown';

		if (this.props.mopidy.connecting || this.props.pusher.connecting){
			icon = 'autorenew';
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
				<Icon name={icon} />{status}
			</span>
		);
	}

	render(){

		var options = (
			<span>
				<button className="no-hover" onClick={e => hashHistory.push(global.baseURL+'settings/debug')}>
					<Icon name="code" />Debug
				</button>
				<a className="no-hover button" href="https://github.com/jaedb/Iris/wiki" target="_blank">
					<Icon name="help" />Help
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
				<Header className="overlay" options={options} uiActions={this.props.uiActions}>
					<Icon name="settings" type="material" />
					Settings
				</Header>

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

					{helpers.isHosted() ? null : <h4 className="underline">Privacy</h4>}

					{helpers.isHosted() ? null : <div className="field checkbox">
						<div className="name">Reporting</div>
						<div className="input">
							<label>
								<input 
									type="checkbox"
									name="allow_reporting"
									checked={ this.props.ui.allow_reporting }
									onChange={ e => this.props.uiActions.set({ allow_reporting: !this.props.ui.allow_reporting })} />
								<span className="label">
									Allow reporting of anonymous usage statistics
								</span>
							</label>
							<div className="description">Anonymous usage data is used to identify errors and potential features that make Iris better for everyone. Read the <a href="https://github.com/jaedb/Iris/wiki/Terms-of-use#privacy-policy" target="_blank">privacy policy</a>.</div>
						</div>
					</div>}

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
							<label>
								<input 
									type="checkbox"
									name="shortkeys_enabled"
									checked={ this.props.ui.shortkeys_enabled }
									onChange={ e => this.props.uiActions.set({ shortkeys_enabled: !this.props.ui.shortkeys_enabled })} />
								<span className="label">
									Enable shortkeys
								</span>
							</label>
						</div>
					</div>

					<div className="field sources-priority">
						<div className="name has-tooltip">
							Sources priority
							<span className="tooltip">Order of searching and search results</span>
						</div>
						<div className="input">
			        		<SourcesPriority
								uri_schemes={this.props.mopidy.uri_schemes ? this.props.mopidy.uri_schemes : []}
								uri_schemes_priority={this.props.ui.uri_schemes_priority ? this.props.ui.uri_schemes_priority : []}
			        			uiActions={this.props.uiActions}
			        		/>
			        	</div>
			        </div>

					<div className="field">
						<div className="name">Artist library URI</div>
						<div className="input">
							<input 
								type="text"
								value={this.state.mopidy_library_artists_uri}
								onChange={e => this.setState({mopidy_library_artists_uri: e.target.value})}
								onBlur={e => this.handleBlur('mopidy', 'library_artists_uri', e.target.value)}
							/>
							<div className="description">
								URI used for collecting library artists
							</div>
						</div>
					</div>

					<div className="field">
						<div className="name">Album library URI</div>
						<div className="input">
							<input 
								type="text"
								value={this.state.mopidy_library_albums_uri}
								onChange={e => this.setState({mopidy_library_albums_uri: e.target.value})}
								onBlur={e => this.handleBlur('mopidy', 'library_albums_uri', e.target.value)}
							/>
							<div className="description">
								URI used for collecting library albums
							</div>
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
				        		{this.props.pusher.version.current} installed {this.props.pusher.version.upgrade_available ? <span className="flag blue">Upgrade available</span> : <span className="flag dark"><Icon type="fontawesome" name="check" className="green-text" />&nbsp; Up-to-date</span>}
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
						</div>
						<br /><br />
						<div>
					        <a className="button" href="https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=james%40barnsley%2enz&lc=NZ&item_name=James%20Barnsley&currency_code=USD&bn=PP%2dDonationsBF%3abtn_donate_LG%2egif%3aNonHosted" target="_blank">
					        	<Icon type="fontawesome" name="paypal" /> Donate
					        </a>
					        <a className="button" href="https://github.com/jaedb/Iris" target="_blank">
					        	<Icon type="fontawesome" name="github" /> GitHub
					        </a>
					        <a className="button" href="http://creativecommons.org/licenses/by-nc/4.0/" target="_blank"><Icon type="fontawesome" name="creative-commons" />&nbsp;Licence</a>
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
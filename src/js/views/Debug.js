
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link, hashHistory } from 'react-router'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import SpotifyAuthenticationFrame from '../components/SpotifyAuthenticationFrame'
import ConfirmationButton from '../components/ConfirmationButton'
import PusherConnectionList from '../components/PusherConnectionList'
import URISchemesList from '../components/URISchemesList'
import VersionManager from '../components/VersionManager'
import Header from '../components/Header'
import Thumbnail from '../components/Thumbnail'

import * as uiActions from '../services/ui/actions'
import * as pusherActions from '../services/pusher/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Debug extends React.Component{

	constructor(props) {
		super(props);
		this.state = {
			mopidy_call: 'playlists.asList',
			mopidy_data: '{}',
			pusher_data: '{"method":"get_config"}',
			access_token: this.props.access_token
		}
	}

	callMopidy(e){
		e.preventDefault()
		this.props.mopidyActions.debug( this.state.mopidy_call, JSON.parse(this.state.mopidy_data) )
	}

	callPusher(e){
		e.preventDefault()
		this.props.pusherActions.debug( JSON.parse(this.state.pusher_data) )
	}

	render(){

		var options = (
			<span>
				<button className="no-hover" onClick={e => hashHistory.push(global.baseURL+'settings')}>
					<FontAwesome name="reply" />&nbsp;
					Back
				</button>
			</span>
		)

		return (
			<div className="view debugger-view">
				<Header icon="cog" title="Debugger" options={options} uiActions={this.props.uiActions} />

				<div className="content-wrapper">

					<h4 className="underline">User interface</h4>
					<form>
						<div className="field checkbox">
							<div className="name">Debug</div>
							<div className="input">
								<label>
									<input 
										type="checkbox"
										name="debug_info"
										checked={ this.props.test_mode }
										onChange={ e => this.props.uiActions.set({ test_mode: !this.props.test_mode })} />
									<span className="label">Test mode</span>
								</label>
								<label>
									<input 
										type="checkbox"
										name="debug_info"
										checked={ this.props.debug_info }
										onChange={ e => this.props.uiActions.set({ debug_info: !this.props.debug_info })} />
									<span className="label">Debug info</span>
								</label>
							</div>
						</div>
						<div className="field checkbox">
							<div className="name">Logging</div>
							<div className="input">
								<label>
									<input 
										type="checkbox"
										name="log_actions"
										checked={ this.props.log_actions }
										onChange={ e => this.props.uiActions.set({ log_actions: !this.props.log_actions })} />
									<span className="label">Log actions</span>
								</label>
								<label>
									<input 
										type="checkbox"
										name="log_mopidy"
										checked={ this.props.log_mopidy }
										onChange={ e => this.props.uiActions.set({ log_mopidy: !this.props.log_mopidy })} />
									<span className="label">Log Mopidy</span>
								</label>
								<label>
									<input 
										type="checkbox"
										name="log_pusher"
										checked={ this.props.log_pusher }
										onChange={ e => this.props.uiActions.set({ log_pusher: !this.props.log_pusher })} />
									<span className="label">Log Pusher</span>
								</label>
							</div>
						</div>
						<div className="field">
							<div className="name"></div>
							<div className="input">
								<a className="button secondary" onClick={e => this.props.uiActions.createNotification('Test notification')}>Create notification</a>
								<a className="button secondary" onClick={e => this.props.uiActions.startProcess('test_process', 'Test process', {total: 100, remaining: 17})}>Start process</a>
								<a className="button secondary" onClick={e => this.props.uiActions.processFinished('test_process')}>Stop process</a>
							</div>
						</div>
					</form>

					<h4 className="underline">Spotify</h4>
					<div className="field">
						<div className="name">Access token</div>
						<div className="input">
							<input
								type="text"
								onChange={e => this.props.spotifyActions.authorizationGranted({access_token: e.target.value})}
								value={this.state.access_token} />
						</div>
					</div>

					<h4 className="underline">Mopidy</h4>
					<form onSubmit={(e) => this.callMopidy(e)}>
						<div className="field">
							<div className="name">Call</div>
							<div className="input">
								<input 
									type="text"
									onChange={ e => this.setState({ mopidy_call: e.target.value })} 
									value={ this.state.mopidy_call } />
							</div>
						</div>
						<div className="field">
							<div className="name">Data</div>
							<div className="input">
								<textarea 
									onChange={ e => this.setState({ mopidy_data: e.target.value })}
									value={ this.state.mopidy_data }>
								</textarea>
							</div>
						</div>
						<div className="field">
							<div className="name"></div>
							<div className="input">
								<button type="submit" className="secondary">Send</button>
							</div>
						</div>
					</form>

					<h4 className="underline">Pusher</h4>
					<form onSubmit={(e) => this.callPusher(e)}>
						<div className="field">
							<div className="name">Examples</div>
							<div className="input">
								<select onChange={ e => this.setState({ pusher_data: e.target.value })}>
									<option value='{"method":"get_config"}'>Get config</option>
									<option value='{"method":"get_version"}'>Get version</option>
									<option value='{"method":"get_connections"}'>Get connections</option>
									<option value='{"method":"get_radio"}'>Get radio</option>
									<option value='{"method":"get_queue_metadata"}'>Get queue metadata</option>
									<option value='{"method":"broadcast","data":{"type":"browser_notification","title":"Testing","body":"This is my message"}}'>Broadcast to all clients</option>
									<option value='{"method":"deliver_message","data":{"to":"CONNECTION_ID_HERE","message":{"type":"browser_notification","title":"Testing","body":"This is my message"}}}'>Broadcast to one client</option>
									<option value='{"method":"set_username","data":{"connection_id":"CONNECTION_ID_HERE","username":"NewUsername"}}'>Change username</option>
									<option value='{"method":"refresh_spotify_token"}'>Refresh Spotify token</option>
									<option value='{"method":"perform_upgrade"}'>Perform upgrade (beta)</option>
								</select>
							</div>
						</div>
						<div className="field">
							<div className="name">Data</div>
							<div className="input">
								<textarea 
									onChange={ e => this.setState({ pusher_data: e.target.value })}
									value={ this.state.pusher_data }>
								</textarea>
							</div>
						</div>
						<div className="field">
							<div className="name"></div>
							<div className="input">
								<button type="submit" className="secondary">Send</button>
							</div>
						</div>
					</form>

					<h4 className="underline">Response</h4>
					<pre>
						{ this.props.debug_response ? JSON.stringify(this.props.debug_response, null, 2) : null }
					</pre>
					
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
		connection_id: state.pusher.connection_id,
		access_token: (state.spotify.access_token ? state.spotify.access_token : ''),
		log_actions: (state.ui.log_actions ? state.ui.log_actions : false),
		log_pusher: (state.ui.log_pusher ? state.ui.log_pusher : false),
		log_mopidy: (state.ui.log_mopidy ? state.ui.log_mopidy : false),
		test_mode: (state.ui.test_mode ? state.ui.test_mode : false),
		debug_info: (state.ui.debug_info ? state.ui.debug_info : false),
		debug_response: state.ui.debug_response
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

export default connect(mapStateToProps, mapDispatchToProps)(Debug)
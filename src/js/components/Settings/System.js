
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

import * as uiActions from '../../services/ui/actions'
import * as pusherActions from '../../services/pusher/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class System extends React.Component{

	constructor(props) {
		super(props);
		this.state = {
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

	render(){
		return (
			<div>

				<h4 className="underline">Server</h4>
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
		ui: state.ui,
		mopidy: state.mopidy,
		pusher: state.pusher
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

export default connect(mapStateToProps, mapDispatchToProps)(System)
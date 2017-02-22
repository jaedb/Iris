
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
			pusher_data: '{"method":"broadcast","data":{"type":"browser_notification","title":"Testing","body":"This is my message"}}'
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

		var actions = (
			<button onClick={e => hashHistory.push(global.baseURL+'settings')}>
				<FontAwesome name="reply" />&nbsp;
				Back
			</button>
		)

		return (
			<div className="view debugger-view">
				<Header icon="cog" title="Debugger" actions={actions}  />

				<section>

					<h4 className="underline">User interface</h4>
					<form>
						<div className="field checkbox">
							<div className="name">Emulation</div>
							<div className="input">
								<label>
									<input 
										type="checkbox"
										name="emulate_touch"
										checked={ this.props.emulate_touch }
										onChange={ e => this.props.uiActions.set({ emulate_touch: !this.props.emulate_touch })} />
									<span className="label">Mouse events as touch</span>
								</label>
							</div>
						</div>
						<div className="field checkbox">
							<div className="name">Debug data</div>
							<div className="input">
								<label>
									<input 
										type="checkbox"
										name="emulate_touch"
										checked={ this.props.log_actions }
										onChange={ e => this.props.uiActions.set({ log_actions: !this.props.log_actions })} />
									<span className="label">Log actions</span>
								</label>
								<label>
									<input 
										type="checkbox"
										name="emulate_touch"
										checked={ this.props.log_mopidy }
										onChange={ e => this.props.uiActions.set({ log_mopidy: !this.props.log_mopidy })} />
									<span className="label">Log Mopidy</span>
								</label>
								<label>
									<input 
										type="checkbox"
										name="emulate_touch"
										checked={ this.props.log_pusher }
										onChange={ e => this.props.uiActions.set({ log_pusher: !this.props.log_pusher })} />
									<span className="label">Log Pusher</span>
								</label>
								<label>
									<input 
										type="checkbox"
										name="debug_info"
										checked={ this.props.debug_info }
										onChange={ e => this.props.uiActions.set({ debug_info: !this.props.debug_info })} />
									<span className="label">Show debug info</span>
								</label>
							</div>
						</div>
					</form>

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
	return {
		connection_id: state.pusher.connection_id,
		emulate_touch: (state.ui.emulate_touch ? state.ui.emulate_touch : false),
		log_actions: (state.ui.log_actions ? state.ui.log_actions : false),
		log_pusher: (state.ui.log_pusher ? state.ui.log_pusher : false),
		log_mopidy: (state.ui.log_mopidy ? state.ui.log_mopidy : false),
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
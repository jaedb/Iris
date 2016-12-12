
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
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
			pusher_call: 'broadcast',
			pusher_data: '{}'
		}
	}

	componentDidMount(){
		if( this.props.pusher.connectionid ){
			var data = {
				action: "notification",
				recipients: [this.props.pusher.connectionid],
				data: {
					title: "Title",
					body: "Test notification",
					icon: "http://lorempixel.com/100/100/nature/"
				}
			}
			this.setState({ pusher_data: JSON.stringify(data) })
		}
	}

	callMopidy(e){
		e.preventDefault()
		console.info('Mopidy Debugger', this.state.mopidy_call, JSON.parse(this.state.mopidy_data) )
		this.props.mopidyActions.debug( this.state.mopidy_call, JSON.parse(this.state.mopidy_data) )
		this.props.uiActions.createNotification( 'Mopidy instruction sent' )
	}

	callPusher(e){
		e.preventDefault()
		console.info('Pusher Debugger', this.state.pusher_call, JSON.parse(this.state.pusher_data) )
		this.props.pusherActions.debug( this.state.pusher_call, JSON.parse(this.state.pusher_data) )
		this.props.uiActions.debugResponse({ status: 1, message: 'Sent', call: this.state.pusher_call, data: this.state.pusher_data })
		this.props.uiActions.createNotification( 'Pusher instruction sent' )
	}

	render(){
		return (
			<div className="view debugger-view">
				<Header icon="cog" title="Debugger" />

				<section>

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

		        </section>

				<section>

					<h4 className="underline">Pusher</h4>
					<form onSubmit={(e) => this.callPusher(e)}>
						<div className="field">
							<div className="name">Call</div>
							<div className="input">
								<input 
									type="text"
									onChange={ e => this.setState({ pusher_call: e.target.value })} 
									value={ this.state.pusher_call } />
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

		        </section>

		        <section>
					<h4 className="underline">Response</h4>
					<pre>
						{ this.props.ui.debug_response ? JSON.stringify(this.props.ui.debug_response, null, 2) : null }
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

export default connect(mapStateToProps, mapDispatchToProps)(Debug)
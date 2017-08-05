
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link, hashHistory } from 'react-router'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import ConfirmationButton from '../ConfirmationButton'
import PusherConnectionList from '../PusherConnectionList'
import URISchemesList from '../URISchemesList'
import VersionManager from '../VersionManager'

import * as uiActions from '../../services/ui/actions'
import * as pusherActions from '../../services/pusher/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class Advanced extends React.Component{

	constructor(props) {
		super(props)
	}

	render(){
		return (
			<div>

				<h4 className="underline">Pusher</h4>

				<div className="field pusher-connections">
					<div className="name">Connections</div>
					<div className="input">
						<span className="text">
		        			<PusherConnectionList />
		        		</span>
		        	</div>
		        </div>
				
				<h4 className="underline">Mopidy extensions</h4>				
				<URISchemesList />
			
				<h4 className="underline">Core</h4>

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
		ui: state.ui
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

export default connect(mapStateToProps, mapDispatchToProps)(Advanced)
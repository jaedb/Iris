
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { hashHistory } from 'react-router'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import TrackList from '../components/TrackList'
import SidebarToggleButton from '../components/SidebarToggleButton'
import Header from '../components/Header'

import * as uiActions from '../services/ui/actions'
import * as pusherActions from '../services/pusher/actions'
import * as spotifyActions from '../services/spotify/actions'
import * as mopidyActions from '../services/mopidy/actions'

class QueueHistory extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.loadHistory() 
	}

	componentWillReceiveProps(nextProps){
		if( !this.props.mopidy_connected && nextProps.mopidy_connected ){
			this.loadHistory(nextProps)
		}
	}

	loadHistory(props = this.props){
		if (props.mopidy_connected){
			this.props.mopidyActions.getQueueHistory()
		}
	}

	render(){
		var options = (
			<span>
				<button onClick={e => hashHistory.push(global.baseURL+'queue')}>
					<FontAwesome name="reply" />&nbsp;
					Back
				</button>
			</span>
		)

		return (
			<div className="view queue-history-view">			
				<Header icon="play" title="Playback history" options={options} uiActions={this.props.uiActions} />

				<section className="list-wrapper">
					<TrackList
						className="queue-history-track-list"
						show_source_icon={true}
						context="history"
						tracks={this.props.queue_history} />
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
		mopidy_connected: state.mopidy.connected,
		queue_history: (state.mopidy.queue_history ? state.mopidy.queue_history : [])
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(QueueHistory)
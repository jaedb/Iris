
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { createStore, bindActionCreators } from 'redux'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'

class DebugInfo extends React.Component{

	constructor(props) {
		super(props);
	}

	renderLoadQueue(){
		if (!this.props.ui.load_queue){
			return null
		}

		var load_queue = this.props.ui.load_queue
		var queue = []

		return (
			<div className="item">
				{queue}
			</div>
		)
	}

	renderLoadQueue(){
		if (!this.props.ui.load_queue){
			return null
		}

		var load_queue = this.props.ui.load_queue
		var queue = []
		for (var key in load_queue){
			if (load_queue.hasOwnProperty(key)){
				queue.push(<div key={key}>{load_queue[key]}</div>)
			}
		}

		if (queue.length > 0){
			return (
				<div className="item">
					<br />
					{queue}
				</div>
			)
		} else {
			return null
		}
	}

	render(){
		return (
			<div className="debug-info">
				<div className="item">
					Albums: {this.props.ui.albums ? Object.keys(this.props.ui.albums).length : '0'}
				</div>
				<div className="item">
					Artists: {this.props.ui.artists ? Object.keys(this.props.ui.artists).length : '0'}
				</div>
				<div className="item">
					Playlists: {this.props.ui.playlists ? Object.keys(this.props.ui.playlists).length : '0'}
				</div>
				<div className="item">
					Tracks: {this.props.ui.tracks ? Object.keys(this.props.ui.tracks).length : '0'}
				</div>
				<div className="item">
					Users: {this.props.ui.users ? Object.keys(this.props.ui.users).length : '0'}
				</div>
				<div className="item">
					Notifications: {this.props.ui.notifications ? Object.keys(this.props.ui.notifications).length : '0'}
				</div>
				<div className="item">
					Processes: {this.props.ui.processes ? Object.keys(this.props.ui.processes).length : '0'}
				</div>
				<div className="item">
					Enqueue batches: {this.props.mopidy.enqueue_uris_batches ? this.props.mopidy.enqueue_uris_batches.length : '0'}
				</div>
				<br />
				<div className="item">
					_testMode: {window._testMode ? 'on' : 'off'}
				</div>
				<div className="item">
					Touch: {helpers.isTouchDevice() ? 'on' : 'off'}
				</div>
				{this.renderLoadQueue()}
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		ui: state.ui,
		mopidy: state.mopidy
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(DebugInfo)
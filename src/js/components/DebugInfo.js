
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { createStore, bindActionCreators } from 'redux'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'

class DebugInfo extends React.Component{

	constructor(props){
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
					Albums: {this.props.core.albums ? Object.keys(this.props.core.albums).length : '0'}
				</div>
				<div className="item">
					Artists: {this.props.core.artists ? Object.keys(this.props.core.artists).length : '0'}
				</div>
				<div className="item">
					Playlists: {this.props.core.playlists ? Object.keys(this.props.core.playlists).length : '0'}
				</div>
				<div className="item">
					Tracks: {this.props.core.tracks ? Object.keys(this.props.core.tracks).length : '0'}
				</div>
				<div className="item">
					Users: {this.props.core.users ? Object.keys(this.props.core.users).length : '0'}
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
					Slim mode: {this.props.ui.slim_mode ? 'on' : 'off'}
				</div>
				<div className="item">
					Test mode: {this.props.ui.test_mode ? 'on' : 'off'}
				</div>
				<div className="item">
					Touch: {helpers.isTouchDevice() ? 'on' : 'off'}
				</div>
				<div className="item">
					Selected tracks: {this.props.ui.selected_tracks.length}<br />
					{
						this.props.ui.selected_tracks.map((track_key, index) => {
							return (
								<div key={track_key+'_'+index}>{track_key}</div>
							)
						})
					}
				</div>
				{this.renderLoadQueue()}
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		core: state.core,
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
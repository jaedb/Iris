
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

	localStorageSize(){
		var data = '';
	
		for (var key in window.localStorage){
			if(window.localStorage.hasOwnProperty(key)){
				data += window.localStorage[key];
			}
		}

		var used = 0;
		var total = 5000;
		if (data !== ''){
			used = ((data.length * 16)/(8 * 1024)).toFixed(2);
		}
		
		return {
			used: used,
			percent: (used / total * 100).toFixed(2)
		};
	}

	renderLoadQueue(){
		if (!this.props.ui.load_queue){
			return null
		}

		var load_queue = this.props.ui.load_queue;
		var queue = [];

		return (
			<div className="debug-info-item">
				{queue}
			</div>
		)
	}

	renderLoadQueue(){
		if (!this.props.ui.load_queue){
			return <div className="debug-info-item grey-text">Nothing loading</div>;
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
				<div className="debug-info-item">
					{queue}
				</div>
			)
		} else {
			return <div className="debug-info-item grey-text">Nothing loading</div>;
		}
	}

	render(){
		var localStorageUsage = this.localStorageSize();

		return (
			<div className="debug-info">

				<div className="debug-info-section">
					<h5>Indexes ({localStorageUsage.percent}%, {localStorageUsage.used}KB)</h5>
					<div className="debug-info-item">
						Albums: {this.props.core.albums ? Object.keys(this.props.core.albums).length : '0'}
					</div>
					<div className="debug-info-item">
						Artists: {this.props.core.artists ? Object.keys(this.props.core.artists).length : '0'}
					</div>
					<div className="debug-info-item">
						Playlists: {this.props.core.playlists ? Object.keys(this.props.core.playlists).length : '0'}
					</div>
					<div className="debug-info-item">
						Tracks: {this.props.core.tracks ? Object.keys(this.props.core.tracks).length : '0'}
					</div>
					<div className="debug-info-item">
						Users: {this.props.core.users ? Object.keys(this.props.core.users).length : '0'}
					</div>
					<div className="debug-info-item">
						Notifications: {this.props.ui.notifications ? Object.keys(this.props.ui.notifications).length : '0'}
					</div>
					<div className="debug-info-item">
						Processes: {this.props.ui.processes ? Object.keys(this.props.ui.processes).length : '0'}
					</div>
					<div className="debug-info-item">
						Enqueue batches: {this.props.mopidy.enqueue_uris_batches ? this.props.mopidy.enqueue_uris_batches.length : '0'}
					</div>
				</div>

				<div className="debug-info-section">
					<h5>Config</h5>
					<div className="debug-info-item">
						Slim mode: {this.props.ui.slim_mode ? 'on' : 'off'}
					</div>
					<div className="debug-info-item">
						Test mode: {this.props.ui.test_mode ? 'on' : 'off'}
					</div>
					<div className="debug-info-item">
						Touch: {helpers.isTouchDevice() ? 'on' : 'off'}
					</div>
					<div className="debug-info-item">
						Selected tracks: {this.props.ui.selected_tracks.length}<br />
						{
							this.props.ui.selected_tracks.map((track_key, index) => {
								return (
									<div key={track_key+'_'+index}>{track_key}</div>
								)
							})
						}
					</div>
				</div>

				<div className="debug-info-section">
					<h5>Load queue</h5>
					{this.renderLoadQueue()}
				</div>

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
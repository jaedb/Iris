
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

		return (
			<div className="item">
				{queue}
			</div>
		)
	}

	render(){
		var touch_state = 'no touch'
		if (this.props.ui.emulate_touch){
			touch_state = 'emulated touch'
		} else if (helpers.isTouchDevice()){
			touch_state = 'touch device'
		}

		return (
			<div className="debug-info">
				<div className="item">
					{this.props.ui.albums ? Object.keys(this.props.ui.albums).length : '0'}&nbsp;albums
				</div>
				<div className="item">
					{this.props.ui.artists ? Object.keys(this.props.ui.artists).length : '0'}&nbsp;artists
				</div>
				<div className="item">
					{this.props.ui.playlists ? Object.keys(this.props.ui.playlists).length : '0'}&nbsp;playlists
				</div>
				<div className="item">
					{this.props.ui.tracks ? Object.keys(this.props.ui.tracks).length : '0'}&nbsp;tracks
				</div>
				<div className="item">
					{this.props.ui.users ? Object.keys(this.props.ui.users).length : '0'}&nbsp;users
				</div>
				<div className="item">
					{this.props.ui.notifications ? Object.keys(this.props.ui.notifications).length : '0'}&nbsp;notifications
				</div>
				<div className="item">
					{touch_state}
				</div>
				{this.renderLoadQueue()}
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		ui: state.ui
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(DebugInfo)
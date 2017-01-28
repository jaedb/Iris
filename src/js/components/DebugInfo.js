
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { createStore, bindActionCreators } from 'redux'

import * as uiActions from '../services/ui/actions'

class DebugInfo extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
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
					{this.props.ui.users ? Object.keys(this.props.ui.users).length : '0'}&nbsp;users
				</div>
				<div className="item">
					{'ontouchstart' in document.documentElement ? 'can touch' : 'no touch'}
				</div>
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
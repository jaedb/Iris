
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { createStore, bindActionCreators } from 'redux'

import FontAwesome from 'react-fontawesome'

import * as pusherActions from '../services/pusher/actions'

class VersionManager extends React.Component{

	constructor(props){
		super(props);
	}

	renderUpgradeButton(){
		return <button className="productive" onClick={() => this.props.pusherActions.startUpgrade()}>Upgrade to { this.props.pusher.version.latest }</button>
		
		if (this.props.pusher.upgrading){
			return (
				<button className="productive working" disabled>
					<FontAwesome name="circle-o-notch" spin />
					&nbsp;
					Upgrading
				</button>
			);
		}

		if (this.props.pusher.version.upgrade_available){
			return <button className="productive" onClick={() => this.props.pusherActions.startUpgrade()}>Upgrade to { this.props.pusher.version.latest }</button>
		}

		return <button className="secondary" disabled>Already up-to-date</button>
	}

	render(){
		return (
			<span className="version-manager">
				<span>{ this.props.pusher.version.current } installed</span>
				{this.renderUpgradeButton()}
			</span>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		pusherActions: bindActionCreators(pusherActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(VersionManager)
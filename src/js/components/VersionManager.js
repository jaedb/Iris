
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { createStore, bindActionCreators } from 'redux'

import FontAwesome from 'react-fontawesome'

import * as pusherActions from '../services/pusher/actions'

class VersionManager extends React.Component{

	constructor(props) {
		super(props);
	}

	renderUpgradeButton(){
		if( this.props.pusher.upgrading ){
			return (
				<button className="outline" disabled>
					<FontAwesome name="circle-o-notch" spin />
					&nbsp;
					Upgrading
				</button>
			);
		}

		if( !this.props.pusher.version.is_root ){
			return <button className="outline" disabled>Not running as root</button>
		}

		if( this.props.pusher.version.upgrade_available ){
			return <button className="primary" onClick={() => this.props.pusherActions.startUpgrade()}>Upgrade to { this.props.pusher.version.latest }</button>
		}

		return <button className="outline" disabled>No updates available</button>
	}

	render(){
		return (
			<span className="version-manager">
				{ this.renderUpgradeButton() }
				<span className="description">{ this.props.pusher.version.current } installed</span>
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
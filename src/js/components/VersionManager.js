
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
				<button disabled>
					<FontAwesome name="circle-o-notch" spin />
					&nbsp;
					Upgrading
				</button>
			);
		}

		if( !this.props.pusher.version.is_root ){
			return <button className="secondary" disabled>Not running as root</button>
		}

		if( this.props.pusher.version.upgrade_available ){
			return <button className="secondary" onClick={() => this.props.pusherActions.performUpgrade()}>Upgrade to { this.props.pusher.version.latest }</button>
		}

		return null;
	}

	render(){
		return (
			<div className="version-manager">
				{ this.renderUpgradeButton() }
				<div className="description">{ this.props.pusher.version.current } currently installed</div>
			</div>
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

import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import * as actions from '../services/ui/actions'

class Notifications extends React.Component{

	constructor(props) {
		super(props)
	}

	render(){
		if( !this.props.notifications ) return null

		return (
			<div className="notifications">
				{
					this.props.notifications.map( notification => {
						if (notification.is_shortcut){
							return (
								<div className="shortcut-notification" key={notification.id}>
									<FontAwesome name={notification.type} />
								</div>
							)
						} else {
							return (
								<div className={notification.type+" notification"} key={notification.id}>
									<FontAwesome name="close" className="close-button" onClick={ e => this.props.actions.removeNotification(notification.id) } />
									{ notification.content }
								</div>
							)
						}
					})
				}
			</div>
		)
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		notifications: state.ui.notifications
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		actions: bindActionCreators(actions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Notifications)
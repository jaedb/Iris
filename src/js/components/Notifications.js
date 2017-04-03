
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

export default class Notifications extends React.Component{

	constructor(props) {
		super(props)
	}

	renderNotifications(){
		if (!this.props.notifications || this.props.notifications.length <= 0) return null

		// we only care about the last notification
		var notification = this.props.notifications[this.props.notifications.length-1]

		if (notification.is_shortcut){
			return (
				<div className="shortcut-notification">
					<FontAwesome name={notification.type} />
				</div>
			)
		} else {
			return (
				<div className={notification.type+" notification"}>
					<FontAwesome name="close" className="close-button" onClick={ e => this.props.uiActions.removeNotification(notification.id) } />
					{ notification.content }
				</div>
			)
		}
	}

	renderLoader(){
		if (!this.props.load_queue){
			return null
		}

		var load_queue = this.props.load_queue
		var is_loading = false
		for (var key in load_queue){
			if (load_queue.hasOwnProperty(key)){
				is_loading = true
				break
			}
		}

		if (is_loading){
			return (
				<div className="notification loading">
					Loading
				</div>
			)
		} else {
			return null
		}
	}

	render(){
		return (
			<div className="notifications">
				{this.renderNotifications()}
				{this.renderLoader()}
			</div>
		)
	}
}
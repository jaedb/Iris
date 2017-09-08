
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

export default class Notifications extends React.Component{

	constructor(props) {
		super(props)
	}

	renderNotifications(){
		if (!this.props.notifications || this.props.notifications.length <= 0) return null

		return (
			<span>
				{
					this.props.notifications.map(notification => {
						switch (notification.type){
							case 'shortcut':
								return (
									<div className="shortcut-notification" key={notification.key}>
										<FontAwesome name={notification.content} />
									</div>
								)

							default:
								return (
									<div className={notification.type+" notification"} key={notification.key} data-key={notification.key}>
										<FontAwesome name="close" className="close-button" onClick={ e => this.props.uiActions.removeNotification(notification.key) } />
										{notification.title ? <h4>{notification.title}</h4> : null}
										<p dangerouslySetInnerHTML={{__html: notification.content}}></p>
									</div>
								)
						}
					})
				}
			</span>
		)
	}

	renderProcess(process){

		var progress = 0;
		if (process.data.total && process.data.remaining){
			progress = ((process.data.total - process.data.remaining) / process.data.total * 100).toFixed()
		}

		switch (process.status){
			case 'running':
				return(
					<div className="process notification" key={process.key}>
						<div className="loader">
							<div className="progress">
								<div className="fill" style={{width: progress+'%'}}></div>
							</div>
						</div>
						{process.message}
						<FontAwesome name="close" className="close-button" onClick={e => {this.props.uiActions.cancelProcess(process.key)}} />
					</div>
				)

			case 'cancelling':
				return(
					<div className="process notification cancelling" key={process.key}>
						<div className="loader"></div>
						Cancelling
					</div>
				)

			case 'cancelled':
			case 'finished':
				return null
		}
	}

	renderProcesses(){
		if (!this.props.processes || this.props.processes.length <= 0) return null

		var processes = []
		for (var key in this.props.processes){
			if (this.props.processes.hasOwnProperty(key)){
				processes.push(this.props.processes[key])
			}
		}

		return (
			<span>
				{processes.map(process => {
					return this.renderProcess(process)
				})}
			</span>
		)
	}

	// do we want the loading of everything to be displayed?
	// not likely...
	renderLoader(){
		if (!this.props.load_queue){
			return null
		}

		var load_queue = this.props.load_queue
		var load_count = 0
		for (var key in load_queue){
			if (load_queue.hasOwnProperty(key)){
				load_count++
			}
		}

		if (load_count > 0){
			var className = "loading "
			if (load_count > 20){
				className += 'high'
			} else if (load_count > 5){
				className += 'medium'
			} else {
				className += 'low'
			}
			return (
				<div className={className}></div>
			)
		} else {
			return null
		}
	}

	render(){
		return (
			<div className="notifications">
				{this.renderLoader()}
				{this.renderNotifications()}
				{this.renderProcesses()}
			</div>
		)
	}
}
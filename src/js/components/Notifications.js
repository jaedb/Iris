
import React, { PropTypes } from 'react';

import Link from './Link';
import Icon from './Icon';

export default class Notifications extends React.Component{

	constructor(props){
		super(props)
	}

	importConfiguration(notification_key, configuration){
		console.log("Importing configuration", configuration);
		var configurations = "";

		if (configuration.interface){
			this.props.uiActions.set(configuration.interface);
		}

		if (configuration.spotify){
			this.props.spotifyActions.importAuthorization(configuration.spotify.authorization, configuration.spotify.me);
		}

		if (configuration.lastfm){
			this.props.lastfmActions.importAuthorization(configuration.lastfm.authorization, configuration.lastfm.me);
		}

		if (configuration.genius){
			this.props.geniusActions.importAuthorization(configuration.genius.authorization, configuration.genius.me);
		}

		this.props.uiActions.removeNotification(notification_key, true);
		this.props.uiActions.createNotification({type: 'info', content: 'Import successful'});
	}

	renderNotifications(){
		if (!this.props.notifications || this.props.notifications.length <= 0) return null

		var notifications = []
		for (var key in this.props.notifications){
			if (this.props.notifications.hasOwnProperty(key)){
				notifications.push(this.props.notifications[key])
			}
		}

		return (
			<span>
				{
					notifications.map(notification => {
						switch (notification.type){
							case 'shortcut':
								return (
									<div className={"notification notification--shortcut"+(notification.closing ? ' closing' : '')} key={notification.key} data-duration={notification.duration}>
										<Icon type="fontawesome" name={notification.content} />
									</div>
								)

							case 'share-configuration-received':
								return (
									<div className={"notification notification--info"} key={notification.key} data-key={notification.key} data-duration={notification.duration}>
										<Icon name="close" className="close-button" onClick={ e => this.props.uiActions.removeNotification(notification.key, true) } />
										
										<h4>Configuration shared</h4>
										<div className="content">
											<p>Another user has shared their configuration with you. This includes:</p>
											<ul>
												{notification.configuration.ui ? <li>User interface</li> : null}
												{notification.configuration.spotify ? <li>Spotify</li> : null}
												{notification.configuration.lastfm ? <li>LastFM</li> : null}
												{notification.configuration.genius ? <li>Genius</li> : null}
											</ul>
											<p>Do you want to import this?</p>
										</div>
										<br />
										<a className="button" onClick={e => this.importConfiguration(notification.key, notification.configuration)}>Import</a>
									</div>
								)

							default:
								return (
									<div className={"notification notification--"+notification.type+(notification.closing ? ' closing' : '')} key={notification.key} data-key={notification.key} data-duration={notification.duration}>
										<Icon name="close" className="close-button" onClick={ e => this.props.uiActions.removeNotification(notification.key, true) } />
										{notification.title ? <h4>{notification.title}</h4> : null}
										{notification.content ? <p className="content" dangerouslySetInnerHTML={{__html: notification.content}}></p> : null}
										{notification.description ? <p className="description" dangerouslySetInnerHTML={{__html: notification.description}}></p> : null }
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
					<div className={"notification notification--process"+(process.closing ? ' closing' : '')} key={process.key}>
						<div className="loader">
							<div className="progress">
								<div className="fill" style={{width: progress+'%'}}></div>
							</div>
						</div>
						{process.message}
						<Icon name="close" className="close-button" onClick={e => {this.props.uiActions.cancelProcess(process.key)}} />
					</div>
				)

			case 'cancelling':
				return(
					<div className={"notification notification--process cancelling"+(process.closing ? ' closing' : '')} key={process.key}>
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
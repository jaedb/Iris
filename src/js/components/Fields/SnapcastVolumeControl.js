
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import VolumeControl from './VolumeControl'
import Icon from '../Icon'

import * as helpers from '../../helpers'
import * as pusherActions from '../../services/pusher/actions'

class SnapcastVolumeControl extends React.Component{

	constructor(props){
		super(props);

		this.state = {
			expanded: false
		}
	}

	componentDidMount(){
		if (this.props.pusher_connected && this.props.snapcast_enabled){
			this.props.pusherActions.getSnapcast();
		}
	}

	renderVolumes(){

		var clients = [];
		for (var key in this.props.snapcast_clients){
			if (this.props.snapcast_clients.hasOwnProperty(key)){
				var client = this.props.snapcast_clients[key];
				//if (client.connected){
					clients.push(client);
				//}
			}
		}

		return (
			<div className="volumes">
				{
					clients.map(client => {
						var name = client.config.name ? client.config.name : client.host.name;

						return (
							<div className="client" key={client.id}>
								<span className="name">
									{name}
								</span>
								<VolumeControl 
									className="client-volume-control"
									volume={client.config.volume.percent}
									mute={client.config.volume.muted}
									onVolumeChange={percent => this.props.pusherActions.setSnapcastClientVolume(client.id, percent)}
									onMuteChange={mute => this.props.pusherActions.setSnapcastClientMute(client.id, mute)}
								/>
							</div>
						)
					})
				}
			</div>
		);
	}

	render(){
		if (this.state.expanded){
			return (
				<span className="snapcast-volume-control">
					<a className="control speakers active" onClick={e => this.setState({expanded: !this.state.expanded})}><Icon name="speaker" /></a>
					{this.renderVolumes()}
				</span>
			);
		} else {
			return (
				<span className="snapcast-volume-control">
					<a className="control speakers" onClick={e => this.setState({expanded: !this.state.expanded})}><Icon name="speaker" /></a>
				</span>
			);
		}
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		snapcast_enabled: state.pusher.config.snapcast_enabled,
		pusher_connected: state.pusher.connected,
		snapcast_clients: state.pusher.snapcast_clients
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		pusherActions: bindActionCreators(pusherActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(SnapcastVolumeControl)
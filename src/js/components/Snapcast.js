
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link, hashHistory } from 'react-router'
import FontAwesome from 'react-fontawesome'

import ArtistSentence from './ArtistSentence'
import VolumeControl from './VolumeControl'
import LatencyControl from './LatencyControl'
import TextField from './Forms/TextField'

import * as helpers from '../helpers'
import * as coreActions from '../services/core/actions'
import * as uiActions from '../services/ui/actions'
import * as pusherActions from '../services/pusher/actions'

class Snapcast extends React.Component{

	constructor(props){
		super(props);
	}

	componentDidMount(){
		if (this.props.pusher_connected && this.props.snapcast_enabled){
			this.props.pusherActions.getSnapcast();
		}
	}

	componentWillReceiveProps(newProps){

		// Just connected
		if (newProps.snapcast_enabled && !this.props.pusher_connected && newProps.pusher_connected){
			this.props.pusherActions.getSnapcast();

		// Just enabled
		// This is the more probable scenario as we don't know if we're enabled until pusher connects
		// and then gets the config from the server
		} else if (!this.props.snapcast_enabled && newProps.snapcast_enabled && newProps.pusher_connected){
			this.props.pusherActions.getSnapcast();
		}
	}

	render(){
		if (!this.props.snapcast_enabled){
			return (
				<div>
					To enable Snapcast, edit your <code>mopidy.conf</code> file
				</div>
			)
		}

		if (!this.props.snapcast_clients || !this.props.snapcast_groups){
			return (
				<div className="lazy-loader body-loader loading">
					<div className="loader"></div>
				</div>
			)
		}

		// Construct a simple array of our groups index
		var groups = [];
		for (var group_id in this.props.snapcast_groups){
			if (this.props.snapcast_groups.hasOwnProperty(group_id)){
				var group = this.props.snapcast_groups[group_id];

				// Merge the group's clients into this group (also as a simple array)
				var clients = [];
				for (var i = 0; i < group.clients_ids.length; i++){
					if (this.props.snapcast_clients.hasOwnProperty(group.clients_ids[i])){
						clients.push(this.props.snapcast_clients[group.clients_ids[i]]);
					}
				}

				groups.push(Object.assign(
					{},
					group,
					{
						clients: clients
					}
				));
			}
		}

		return (
			<div className="snapcast">
				{
					groups.map(group => {
						return (
							<div className="list group" key={group.id}>
								<div className="list-item header">
									<div className="col name">
										{group.name ? group.name : 'Untitled group'}
									</div>
									<div className="col volume">
										Volume
									</div>
									<div className="col latency">
										Latency
									</div>
								</div>
								{
									group.clients.map(client => {
										var name = client.config.name ? client.config.name : client.host.name;
										return (
											<div className="list-item client" key={client.id}>
												<div className="col name">
													<TextField
														onChange={value => this.props.pusherActions.setSnapcastClientName(client.id, value)}
														value={name}
													/>
													{client.connected ? <span className="status has-tooltip">
														<FontAwesome className="green-text" name="circle" />
														<span className="tooltip">Connected</span>
														</span> : <span className="status has-tooltip">
														<FontAwesome className="grey-text" name="circle" />
														<span className="tooltip">Not connected</span>
														</span>}
												</div>
												<div className="col volume">
													<VolumeControl 
														volume={client.config.volume.percent}
														mute={client.config.volume.muted}
														onVolumeChange={percent => this.props.pusherActions.setSnapcastClientVolume(client.id, percent)}
														onMuteChange={mute => this.props.pusherActions.setSnapcastClientMute(client.id, mute)}
													/>
													<TextField
														className="tiny"
														onChange={value => this.props.pusherActions.setSnapcastClientVolume(client.id, parseInt(value))}
														value={client.config.volume.percent}
													/>
												</div>
												<div className="col latency">
													<LatencyControl 
														max="100"
														value={client.config.latency}
														onChange={value => this.props.pusherActions.setSnapcastClientLatency(client.id, parseInt(value))}
													/>
													<TextField
														className="tiny"
														onChange={value => this.props.pusherActions.setSnapcastClientLatency(client.id, parseInt(value))}
														value={String(client.config.latency)}
													/>
												</div>
											</div>
										);
									})
								}
							</div>
						);
					})
				}
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		snapcast_enabled: state.pusher.config.snapcast_enabled,
		pusher_connected: state.pusher.connected,
		snapcast_groups: state.pusher.snapcast_groups,
		snapcast_clients: state.pusher.snapcast_clients
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Snapcast)
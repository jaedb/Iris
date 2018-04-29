
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link, hashHistory } from 'react-router'
import FontAwesome from 'react-fontawesome'

import ArtistSentence from './ArtistSentence'
import VolumeControl from './Fields/VolumeControl'
import LatencyControl from './Fields/LatencyControl'
import TextField from './Fields/TextField'
import DropdownField from './Fields/DropdownField'

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

		var streams = [];
		for (var key in this.props.snapcast_streams){
			if (this.props.snapcast_streams.hasOwnProperty(key)){
				streams.push(this.props.snapcast_streams[key]);
			}
		}

		// Construct a simple array of our groups index
		var groups = [];
		var groups_dropdown = [];
		for (var group_id in this.props.snapcast_groups){
			if (this.props.snapcast_groups.hasOwnProperty(group_id)){
				var group = this.props.snapcast_groups[group_id];

				// Append to our groups dropdown source array
				groups_dropdown.push({
					label: (group.name ? group.name : group.stream_id),
					value: group.id
				});

				// Merge the group's clients into this group (also as a simple array)
				var clients = [];
				for (var i = 0; i < group.clients_ids.length; i++){
					if (this.props.snapcast_clients.hasOwnProperty(group.clients_ids[i])){
						var client = this.props.snapcast_clients[group.clients_ids[i]];
						if (client.connected || this.props.snapcast_show_disconnected_clients){
							clients.push(client);
						}
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

				<div className="field checkbox">
					<div className="input">
						<button onClick={e => this.props.pusherActions.getSnapcast()}>Refresh</button>
						<label>
							<input 
								type="checkbox"
								name="snapcast_show_disconnected_clients"
								checked={this.props.snapcast_show_disconnected_clients}
								onChange={e => this.props.uiActions.set({ snapcast_show_disconnected_clients: !this.props.snapcast_show_disconnected_clients })} />
							<span className="label">
								Show disconnected clients
							</span>
						</label>
					</div>
				</div>

				{
					groups.map(group => {
						return (
							<div className="group" key={group.id}>
								<h3 className="name">
									{group.name ? group.name : group.id} ({group.stream_id})
								</h3>
								<div className="field dropdown">
									<div className="name">
										Source
									</div>
									<div className="input">										
										<select onChange={e => this.props.pusherActions.setSnapcastGroupStream(group.id, e.target.value)} value={group.stream_id}>
											{
												streams.map(stream => {
													return (
														<option value={stream.id} key={stream.id}>
															{stream.id} ({stream.status})
														</option>
													);
												})
											}
										</select>
									</div>
								</div>
								<div className="field">
									<div className="name">
										Clients
									</div>
									<div className="input">	
										<div className="list clients">
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
																<div className="tools">
																	{client.connected ? null : <FontAwesome className="disconnected red-text" name="plug" />}
																	<DropdownField icon="cog" name="Group" no_label value={group.id} options={groups_dropdown} handleChange={value => {this.props.pusherActions.setSnapcastClientGroup(client.id, value); this.props.uiActions.hideContextMenu()}} />
																</div>
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
									</div>
								</div>
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
		snapcast_show_disconnected_clients: (state.ui.snapcast_show_disconnected_clients !== undefined ? state.ui.snapcast_show_disconnected_clients : false),
		snapcast_streams: state.pusher.snapcast_streams,
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
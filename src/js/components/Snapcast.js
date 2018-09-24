
import React, { PropTypes } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { Link, hashHistory } from 'react-router';

import VolumeControl from './Fields/VolumeControl';
import LatencyControl from './Fields/LatencyControl';
import TextField from './Fields/TextField';
import DropdownField from './Fields/DropdownField';
import Icon from './Icon';

import * as helpers from '../helpers';
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as pusherActions from '../services/pusher/actions';
import * as snapcastActions from '../services/snapcast/actions';

class Snapcast extends React.Component{

	constructor(props){
		super(props);

		this.state = {
			clients_expanded: []
		}
	}

	componentDidMount(){
		if (this.props.pusher_connected && this.props.snapcast_enabled){
			this.props.snapcastActions.getServer();
		}
	}

	componentWillReceiveProps(newProps){

		// Just connected
		if (newProps.snapcast_enabled && !this.props.pusher_connected && newProps.pusher_connected){
			this.props.snapcastActions.getServer();

		// Just enabled
		// This is the more probable scenario as we don't know if we're enabled until pusher connects
		// and then gets the config from the server
		} else if (!this.props.snapcast_enabled && newProps.snapcast_enabled && newProps.pusher_connected){
			this.props.snapcastActions.getServer();
		}
	}

	toggleClientExpanded(client_id){
		var clients_expanded = this.state.clients_expanded;
		var index = clients_expanded.indexOf(client_id);

		if (index >= 0){
			clients_expanded.splice(index, 1);
		} else {
			clients_expanded.push(client_id);
		}

		this.setState({clients_expanded: clients_expanded});
	}

	renderClientsList(group, groups){

		if (!group.clients || group.clients.length <= 0){
			return (
				<div className="text grey-text">
					No clients
				</div>
			);
		}

		return (
			<div className="list snapcast__clients">
				{
					group.clients.map(client => {
						var name = client.config.name ? client.config.name : client.host.name;
						var groups_dropdown = [];
						for (var i = 0; i < groups.length; i++){

							// Don't add our existing group
							if (groups[i].id !== group.id){
								groups_dropdown.push({
									label: (groups[i].name ? groups[i].name : 'Group '+groups[i].id.substring(0,3)),
									value: groups[i].id
								});
							}
						}

						// And append with 'New group' (which is actually
						// the existing group and middleware handles the behavior shift)
						groups_dropdown.push({
							label: 'New group',
							value: group.id,
							className: 'grey-text'
						});

						var class_name = "list__item snapcast__client";
						if (client.connected){
							class_name += " snapcast__client--connected";
						} else {
							class_name += " snapcast__client--disconnected";
						}

						if (this.state.clients_expanded.includes(client.id)){
							return (
								<div className={class_name+" snapcast__client--expanded"} key={client.id}>
									<div className="snapcast__client__expander" onClick={e => this.toggleClientExpanded(client.id)}>
										<Icon name="check" />
									</div>
									<div className="snapcast__client__name">
										<TextField
											onChange={value => this.props.snapcastActions.setClientName(client.id, value)}
											value={name}
										/>
										<DropdownField 
											className="snapcast__client__group-field" 
											icon="folder" 
											name="Group" 
											no_label
											no_status_icon
											value={group.id} 
											options={groups_dropdown} 
											uid={group.id+"_"+client.id}
											handleChange={value => {this.props.snapcastActions.setClientGroup(client.id, value); this.props.uiActions.hideContextMenu()}} 
										/>
									</div>
									<div className="snapcast__client__volume">
										<VolumeControl 
											className="snapcast__client__volume-control"
											volume={client.config.volume.percent}
											mute={client.config.volume.muted}
											onVolumeChange={percent => this.props.snapcastActions.setClientVolume(client.id, percent)}
											onMuteChange={mute => this.props.snapcastActions.setClientMute(client.id, mute)}
										/>
									</div>
									<div className="snapcast__client__latency">
										<LatencyControl 
											max="100"
											value={client.config.latency}
											onChange={value => this.props.snapcastActions.setClientLatency(client.id, parseInt(value))}
										/>
										<TextField
											className="tiny"
											type="number"
											onChange={value => this.props.snapcastActions.setClientLatency(client.id, parseInt(value))}
											value={String(client.config.latency)}
										/>
									</div>
								</div>
							);
						} else {
							return (
								<div className={class_name} key={client.id}>
									<div className="snapcast__client__expander" onClick={e => this.toggleClientExpanded(client.id)}>
										<Icon name="edit" />
									</div>
									<div className="snapcast__client__name">
										{name}
									</div>
								</div>
							);
						}
					})
				}
			</div>
		);
	}

	render(){
		if (!this.props.snapcast_enabled){
			return (
				<p className="message warning">To enable Snapcast, edit your <code>mopidy.conf</code> file</p>
			)
		}

		if (!this.props.clients || !this.props.groups){
			return (
				<div className="lazy-loader body-loader loading">
					<div className="loader"></div>
				</div>
			)
		}

		var streams = [];
		for (var key in this.props.streams){
			if (this.props.streams.hasOwnProperty(key)){
				streams.push(this.props.streams[key]);
			}
		}

		// Construct a simple array of our groups index
		var groups = [];
		for (var group_id in this.props.groups){
			if (this.props.groups.hasOwnProperty(group_id)){
				var group = this.props.groups[group_id];

				// Merge the group's clients into this group (also as a simple array)
				var clients = [];
				for (var i = 0; i < group.clients_ids.length; i++){
					if (this.props.clients.hasOwnProperty(group.clients_ids[i])){
						var client = this.props.clients[group.clients_ids[i]];
						if (client.connected || this.props.show_disconnected_clients){
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
					<div className="name">
						Display
					</div>
					<div className="input">
						<button 
							className="small"
							onClick={e => this.props.snapcastActions.getServer()}>
								Refresh
						</button>
						<label>
							<input 
								type="checkbox"
								name="show_disconnected_clients"
								checked={this.props.show_disconnected_clients}
								onChange={e => this.props.uiActions.set({ snapcast_show_disconnected_clients: !this.props.show_disconnected_clients })} />
							<span className="label">
								Show disconnected clients
							</span>
						</label>
					</div>
				</div>

				<div className="snapcast__groups">
					{
						groups.map(group => {

							// Average our clients' volume for an overall group volume
							var group_volume = 0;
							for (var i = 0; i < group.clients.length; i++){
								var client = group.clients[i];
								group_volume += client.config.volume.percent;
							}
							group_volume = group_volume / group.clients.length;

							return (
								<div className="snapcast__group" key={group.id}>
									<div className="field">
										<div className="name">
											Name
										</div>
										<div className="input">	
											<div className="text">
												{group.name ? group.name : 'Group '+group.id.substring(0,3)} &nbsp;
												<span className="grey-text">({group.id})</span>
											</div>
										</div>
									</div>
									<div className="field dropdown">
										<div className="name">
											Stream
										</div>
										<div className="input">										
											<select onChange={e => this.props.snapcastActions.setGroupStream(group.id, e.target.value)} value={group.stream_id}>
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
											Volume
										</div>
										<div className="input">	
											<VolumeControl 
												className="snapcast__group__volume-control"
												volume={group_volume}
												mute={group.muted}
												onVolumeChange={(percent, old_percent) => this.props.snapcastActions.setGroupVolume(group.id, percent, old_percent)}
												onMuteChange={mute => this.props.snapcastActions.setGroupMute(group.id, mute)}
											/>
										</div>
									</div>
									<div className="field">
										<div className="name">
											Clients
										</div>
										<div className="input">	
											{this.renderClientsList(group, groups)}
										</div>
									</div>
								</div>
							);
						})
					}
				</div>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		snapcast_enabled: state.pusher.config.snapcast_enabled,
		pusher_connected: state.pusher.connected,
		show_disconnected_clients: (state.ui.snapcast_show_disconnected_clients !== undefined ? state.ui.snapcast_show_disconnected_clients : false),
		streams: (state.snapcast.streams ? state.snapcast.streams : null),
		groups: (state.snapcast.groups ? state.snapcast.groups : null),
		clients: (state.snapcast.clients ? state.snapcast.clients : null)
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		snapcastActions: bindActionCreators(snapcastActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Snapcast)
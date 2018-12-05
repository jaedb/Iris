
import React, { PropTypes } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { hashHistory } from 'react-router'
import Link from './Link';

import VolumeControl from './Fields/VolumeControl';
import MuteControl from './Fields/MuteControl';
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

		if (!this.props.show_disconnected_clients && group.clients){
			var clients = helpers.applyFilter('connected', true, group.clients);
		} else {
			var clients = group.clients;
		}

		if (!clients || clients.length <= 0){
			return (
				<div className="text mid_grey-text">
					No clients
				</div>
			);
		}

		return (
			<div className="list snapcast__clients">
				{
					clients.map(client => {
						var class_name = "list__item snapcast__client";
						if (client.connected){
							class_name += " snapcast__client--connected";
						} else {
							class_name += " snapcast__client--disconnected";
						}

						if (this.state.clients_expanded.includes(client.id)){
							return (
								<div className={class_name+" snapcast__client--expanded"} key={client.id}>
									<div className="snapcast__client__header" onClick={e => this.toggleClientExpanded(client.id)}>
										{client.name}
										<div className="snapcast__client__header__icons">
											{!client.connected ? <Icon name="power_off" className="disconnected" /> : null}
											<Icon name="expand_less" />
										</div>
									</div>
									<div className="snapcast__client__details">
										<div className="field">
											<div className="name">
												Name
											</div>
											<div className="input">
												<TextField
													onChange={value => this.props.snapcastActions.setClientName(client.id, value)}
													value={client.name}
												/>
											</div>
										</div>
										<div className="field dropdown">
											<div className="name">
												Group
											</div>
											<div className="input">										
												<select onChange={e => this.props.snapcastActions.setClientGroup(client.id, e.target.value)} value={group.id}>
													{
														groups.map(group => {
															return (
																<option value={group.id} key={group.id}>
																	{group.name ? group.name : 'Group '+group.id.substring(0,3)}
																</option>
															);
														})
													}
													<option value={group.id}>
														New group
													</option>
												</select>
											</div>
										</div>
										<div className="snapcast__client__volume field">
											<div className="name">
												Volume
											</div>
											<div className="input">
												<MuteControl 
													className="snapcast__client__mute-control"
													mute={client.mute}
													onMuteChange={mute => this.props.snapcastActions.setClientMute(client.id, mute)}
												/>
												<VolumeControl 
													className="snapcast__client__volume-control"
													volume={client.volume}
													onVolumeChange={percent => this.props.snapcastActions.setClientVolume(client.id, percent)}
												/>
											</div>
										</div>
										<div className="snapcast__client__latency field">
											<div className="name">
												Latency
											</div>
											<div className="input">
												<LatencyControl 
													max="100"
													value={client.latency}
													onChange={value => this.props.snapcastActions.setClientLatency(client.id, parseInt(value))}
												/>
												<TextField
													className="tiny"
													type="number"
													onChange={value => this.props.snapcastActions.setClientLatency(client.id, parseInt(value))}
													value={String(client.latency)}
												/>
											</div>
										</div>
									</div>
								</div>
							);
						} else {
							return (
								<div className={class_name+" snapcast__client--collapsed"} key={client.id}>
									<div className="snapcast__client__header" onClick={e => this.toggleClientExpanded(client.id)}>
										{client.name}
										<div className="snapcast__client__header__icons">
											{!client.connected ? <Icon name="power_off" className="disconnected" /> : null}
											<Icon name="expand_more" />
										</div>
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

		var streams = [];
		for (var id in this.props.streams){
			if (this.props.streams.hasOwnProperty(id)){
				streams.push(this.props.streams[id]);
			}
		}

		var groups = [];
		for (var id in this.props.groups){
			if (this.props.groups.hasOwnProperty(id)){
				groups.push(this.props.groups[id]);
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
							className="button button--small"
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

							group = helpers.collate(group, {clients: this.props.clients});

							// Average our clients' volume for an overall group volume
							var group_volume = 0;
							for (var i = 0; i < group.clients.length; i++){
								var client = group.clients[i];
								group_volume += client.volume;
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
												{group.name} &nbsp;
												<span className="mid_grey-text">({group.id})</span>
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
											<MuteControl 
												className="snapcast__group__mute-control"
												mute={group.muted}
												onMuteChange={mute => this.props.snapcastActions.setGroupMute(group.id, mute)}
											/>
											<VolumeControl 
												className="snapcast__group__volume-control"
												volume={group_volume}
												onVolumeChange={(percent, old_percent) => this.props.snapcastActions.setGroupVolume(group.id, percent, old_percent)}
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
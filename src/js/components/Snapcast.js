
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link, hashHistory } from 'react-router'
import FontAwesome from 'react-fontawesome'

import ArtistSentence from './ArtistSentence'
import VolumeControl from './VolumeControl'

import * as helpers from '../helpers'
import * as coreActions from '../services/core/actions'
import * as uiActions from '../services/ui/actions'
import * as pusherActions from '../services/pusher/actions'

class Snapcast extends React.Component{

	constructor(props){
		super(props);

		this.state = {
			editing_client: null
		}
	}

	componentDidMount(){
		if (this.props.pusher_connected){
			this.props.pusherActions.getSnapcast();
		}
	}

	componentWillReceiveProps(newProps){
		if (!this.props.pusher_connected && newProps.pusher_connected){
			this.props.pusherActions.getSnapcast();
		}
	}

	saveEditingClient(){
		this.props.pusherActions.setSnapcastClientName(this.state.editing_client.id, this.state.editing_client.name);
		this.setState({editing_client: null});
	}

	render(){
		if (!this.props.snapcast_clients || !this.props.snapcast_groups){
			return null;
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
							<div className="group" key={group.id}>
								<div className="inner">
									<div className="name">
										{group.name ? group.name : 'Untitled group'}
									</div>
								</div>
								<div className="clients">
									{
										group.clients.map(client => {
											var name = client.config.name ? client.config.name : client.host.name;
											return (
												<div className={"client "+(client.connected ? 'connected' : 'disconnected')} key={client.id}>
													<div className="inner">
														<div className="name">
															{name} {!client.connected ? '(disconnected)' : null}
														</div>
														<div className="controls">
															<div className="control edit" onClick={e => this.props.uiActions.openModal('edit_snapcast_client',{id: client.id})}>
																<FontAwesome name="cog" />
															</div>
															<VolumeControl 
																volume={client.config.volume.percent}
																mute={client.config.volume.muted}
																onVolumeChange={percent => this.props.pusherActions.setSnapcastClientVolume(client.id, client.config.volume.muted, percent)}
																onMuteChange={mute => this.props.pusherActions.setSnapcastClientVolume(client.id, mute, client.config.volume.percent)}
															/>
														</div>
													</div>
												</div>
											);
										})
									}
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
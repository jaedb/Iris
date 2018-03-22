
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link, hashHistory } from 'react-router'
import FontAwesome from 'react-fontawesome'

import ArtistSentence from './ArtistSentence'
import * as helpers from '../helpers'
import * as coreActions from '../services/core/actions'
import * as uiActions from '../services/ui/actions'
import * as pusherActions from '../services/pusher/actions'

class Snapcast extends React.Component{

	constructor(props){
		super(props)
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

	renderMuteToggle(client){
		if (client.config.volume.muted){
			return (
				<span className="mute-button" onClick={e => this.props.pusherActions.setSnapcastClientVolume(client.id, false, client.config.volume.percent)}>
					<FontAwesome name="volume-off" />
				</span>
			);
		} else {
			return (
				<span className="mute-button" onClick={e => this.props.pusherActions.setSnapcastClientVolume(client.id, true, client.config.volume.percent)}>
					<FontAwesome name="volume-up" />
				</span>
			);
		}
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
								{group.muted ? <FontAwesome name="volume-off" /> : <FontAwesome name="volume-up" />} {group.name ? group.name : 'Untitled'}
								<div className="clients">
									{
										group.clients.map(client => {
											return (
												<div className="client" key={client.id}>
													{this.renderMuteToggle(client)}
													{client.config.name ? client.config.name : client.host.name} {client.config.volume.percent}
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
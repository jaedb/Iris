
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import VolumeControl from './VolumeControl'
import Icon from '../Icon'

import * as helpers from '../../helpers'
import * as coreActions from '../../services/core/actions'
import * as pusherActions from '../../services/pusher/actions'

class OutputControl extends React.Component{

	constructor(props){
		super(props);

		this.state = {
			expanded: false
		}

		this.handleClick = this.handleClick.bind(this);
	}

	handleClick(e){
		if ($(e.target).closest('.output-control').length <= 0){
			this.setExpanded(false);
		}
	}

	setExpanded(expanded = !this.state.expanded){
		if (expanded){
			this.setState({expanded: expanded});
			window.addEventListener("click", this.handleClick, false);

			// Re-check our snapcast clients
			// TODO: Once we have push events, remove this as it'll (marginally)
			// slow down the reveal/render
			if (this.props.pusher_connected && this.props.snapcast_enabled){
				this.props.pusherActions.getSnapcast();
			}
		} else {
			this.setState({expanded: expanded});
			window.removeEventListener("click", this.handleClick, false);
		}
	}

	renderOutputs(){

		var clients = [];
		for (var key in this.props.snapcast_clients){
			if (this.props.snapcast_clients.hasOwnProperty(key)){
				var client = this.props.snapcast_clients[key];
				if (client.connected){
					clients.push(client);
				}
			}
		}

		var snapcast_clients = null;
		if (clients.length > 0){
			snapcast_clients = (
				<div>	
					{
						clients.map(client => {
							var name = client.config.name ? client.config.name : client.host.name;

							return (
								<div className="output snapcast-output" key={client.id}>
									<div className="name">
										{name}
									</div>
									<VolumeControl 
										className="client-volume-control"
										volume={client.config.volume.percent}
										mute={client.config.volume.muted}
										onVolumeChange={percent => this.props.pusherActions.setSnapcastClientVolume(client.id, percent)}
										onMuteChange={mute => this.props.pusherActions.setSnapcastClientMute(client.id, mute)}
									/>
								</div>
							);
						})
					}
				</div>
			);
		}

		var local_streaming = null;
		if (this.props.http_streaming_enabled){
			local_streaming = (
				<div className="output icecast-output">
					<div className="actions">
						<span className="action" onClick={e => this.props.coreActions.cachebustHttpStream()}>
							<Icon name="refresh" />
						</span>
					</div>
					<div className="name">
						Local browser
					</div>
					<VolumeControl 
						className="client-volume-control"
						volume={this.props.http_streaming_volume}
						mute={this.props.http_streaming_mute}
						onVolumeChange={percent => this.props.coreActions.set({http_streaming_volume: percent})}
						onMuteChange={mute => this.props.coreActions.set({http_streaming_mute: mute})}
					/>
				</div>
			);
		}

		if (!local_streaming && !snapcast_clients){
			return (
				<div className="outputs">
					<p className="no-results">No outputs</p>
				</div>
			);
		} else {
			return (
				<div className="outputs">
					{local_streaming}
					{snapcast_clients}
				</div>
			);
		}
	}

	render(){
		if (this.state.expanded){
			return (
				<span className="output-control">
					<a className="control speakers active" onClick={e => this.setExpanded()}><Icon name="speaker" /></a>
					{this.renderOutputs()}
				</span>
			);
		} else {
			
			// No customisable outputs
			if (!this.props.http_streaming_enabled && !this.props.snapcast_enabled){
				return (
					<span className="output-control disabled">
						<a className="control speakers"><Icon name="speaker" /></a>
					</span>
				);
			} else {
				return (
					<span className="output-control">
						<a className="control speakers" onClick={e => this.setExpanded()}><Icon name="speaker" /></a>
					</span>
				);
			}
		}
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		http_streaming_enabled: state.core.http_streaming_enabled,
		http_streaming_volume: state.core.http_streaming_volume,
		http_streaming_mute: state.core.http_streaming_mute,
		snapcast_enabled: (state.pusher.config ? state.pusher.config.snapcast_enabled : null),
		pusher_connected: state.pusher.connected,
		snapcast_clients: state.pusher.snapcast_clients
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(OutputControl)

import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import VolumeControl from './VolumeControl'
import SnapcastPowerButton from './SnapcastPowerButton'
import Icon from '../Icon'

import * as helpers from '../../helpers';
import * as coreActions from '../../services/core/actions';
import * as pusherActions from '../../services/pusher/actions';
import * as snapcastActions from '../../services/snapcast/actions';

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
				//if (client.connected){
					clients.push(client);
				//}
			}
		}

		var snapcast_clients = null;
		if (clients.length > 0){
			snapcast_clients = (
				<div>	
					{
						clients.map(client => {
							return (
								<div className="output-control__item outputs__item--snapcast" key={client.id}>
									<div className="output-control__item__name">
										{client.name}
									</div>
									<div className="output-control__item__details">
										{client.commands && client.commands.power_on ? <SnapcastPowerButton
											className="output-control__item__power output-control__item__power--on" 
											client={client}
										/> : null}
										<VolumeControl 
											className="output-control__item__volume"
											volume={client.volume}
											mute={client.mute}
											onVolumeChange={percent => this.props.snapcastActions.setClientVolume(client.id, percent)}
											onMuteChange={mute => this.props.snapcastActions.setClientMute(client.id, mute)}
										/>
									</div>
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
				<div className="output-control__item outputs__item--icecast">
					<div className="output-control__item__actions">
						<span className="output-control__item__action" onClick={e => this.props.coreActions.cachebustHttpStream()}>
							<Icon name="refresh" />
						</span>
					</div>
					<div className="output-control__item__name">
						Local browser
					</div>
					<div className="output-control__item__details">
						<VolumeControl 
							className="output-control__item__volume"
							volume={this.props.http_streaming_volume}
							mute={this.props.http_streaming_mute}
							onVolumeChange={percent => this.props.coreActions.set({http_streaming_volume: percent})}
							onMuteChange={mute => this.props.coreActions.set({http_streaming_mute: mute})}
						/>
					</div>
				</div>
			);
		}

		if (!local_streaming && !snapcast_clients){
			return (
				<div className="output-control__items">
					<p className="no-results">No outputs</p>
				</div>
			);
		} else {
			return (
				<div className="output-control__items">
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
		snapcast_clients: state.snapcast.clients
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		snapcastActions: bindActionCreators(snapcastActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(OutputControl)
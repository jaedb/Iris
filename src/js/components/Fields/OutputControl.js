
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import VolumeControl from './VolumeControl'
import MuteControl from './MuteControl'
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
		if (!this.props.force_expanded && $(e.target).closest('.output-control').length <= 0){
			this.setExpanded(false);
		}
	}

	componentWillReceiveProps(nextProps){

		// When force expanded triggered
		if (!this.props.force_expanded && nextProps.force_expanded){
			this.setExpanded(true, nextProps);
		}
	}

	setExpanded(expanded = !this.state.expanded, props = this.props){
		if (expanded){
			this.setState({expanded: expanded});
			window.addEventListener("click", this.handleClick, false);

			// Re-check our snapcast clients
			// TODO: Once we have push events, remove this as it'll (marginally)
			// slow down the reveal/render
			if (props.pusher_connected && props.snapcast_enabled){
				this.props.snapcastActions.getServer();
			}
		} else {
			this.setState({expanded: expanded});
			window.removeEventListener("click", this.handleClick, false);
		}
	}

	renderOutputs(){
		var has_outputs = false;

		var clients = [];
		for (var key in this.props.snapcast_clients){
			if (this.props.snapcast_clients.hasOwnProperty(key)){
				var client = this.props.snapcast_clients[key];
				if (client.connected || this.props.show_disconnected_clients){
					clients.push(client);
				}
			}
		}

		var snapcast_clients = null;
		if (clients.length > 0){
			has_outputs = true;
			snapcast_clients = (
				<div>	
					{
						clients.map(client => {
							return (
								<div className="output-control__item outputs__item--snapcast" key={client.id}>
									<div className="output-control__item__name">
										{client.name}
									</div>
									<div className="output-control__item__controls">
										<MuteControl
											className="output-control__item__mute"
											noTooltip={true}
											mute={client.mute}
											onMuteChange={mute => this.props.snapcastActions.setClientMute(client.id, mute)}
										/>
										<VolumeControl 
											className="output-control__item__volume"
											volume={client.volume}
											mute={client.mute}
											onVolumeChange={percent => this.props.snapcastActions.setClientVolume(client.id, percent)}
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
			has_outputs = true;
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

		var commands = null;
		if (this.props.pusher_commands){

			var commands_items = [];
			for (var key in this.props.pusher_commands){
				if (this.props.pusher_commands.hasOwnProperty(key)){
					commands_items.push(this.props.pusher_commands[key]);
				}
			}

			if (commands_items.length > 0){
				has_outputs = true;
				commands = (
					<div className="output-control__item output-control__item--commands commands">
						{
							commands_items.map(command => {
								return (
									<div 
										key={command.id}
										className="commands__item commands__item--interactive"
										onClick={e => this.props.pusherActions.sendCommand(command.id)}>
											<Icon className="commands__item__icon" name={command.icon} />
											<span className={command.colour+'-background commands__item__background'}></span>
									</div>
								);
							})
						}
					</div>
				);
			}
		}

		if (!has_outputs){
			return (
				<div className="output-control__items output-control__items--no-results">
					<p className="no-results">No outputs</p>
				</div>
			);
		} else {
			return (
				<div className="output-control__items">
					{commands}
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
		pusher_connected: state.pusher.connected,
		snapcast_enabled: (state.pusher.config ? state.pusher.config.snapcast_enabled : null),
		show_disconnected_clients: (state.ui.snapcast_show_disconnected_clients !== undefined ? state.ui.snapcast_show_disconnected_clients : false),
		snapcast_clients: state.snapcast.clients,
		pusher_commands: (state.pusher.commands ? state.pusher.commands : {})
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
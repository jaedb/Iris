
import React from 'react';

import Icon from '../Icon';
import * as helpers from '../../helpers';

export default class MuteControl extends React.Component{

	constructor(props){
		super(props);
	}

	render(){
		if (this.props.mute){
			return (
				<a className={"control mute-control "+(this.props.noTooltip ? "" : "tooltip ")+(this.props.className ? this.props.className : "")} onClick={() => this.props.onMuteChange(false)}>
					<Icon className="red-text" name="volume_off" />
					{this.props.noTooltip ? null : <span className="tooltip__content">Unmute</span>}
				</a>
			)
		} else {
			return (
				<a className={"control mute-control "+(this.props.noTooltip ? "" : "tooltip ")+(this.props.className ? this.props.className : "")} onClick={() => this.props.onMuteChange(true)}>
					<Icon className="muted" name="volume_mute" />
					{this.props.noTooltip ? null : <span className="tooltip__content">Mute</span>}
				</a>
			)
		}
	}
}

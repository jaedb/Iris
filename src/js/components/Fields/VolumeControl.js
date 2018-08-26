
import React from 'react';

import Icon from '../Icon';
import * as helpers from '../../helpers';

export default class VolumeControl extends React.Component{

	constructor(props){
		super(props);
		
		this.handleChange = helpers.throttle(this.handleChange.bind(this), 100);
	}

	handleChange(value){
		this.props.onVolumeChange(value, this.props.volume);
	}

	renderMuteButton(){
		if (this.props.mute){
			return (
				<a className="control mute-control has-tooltip" onClick={() => this.props.onMuteChange(false)}>
					<Icon className="red-text" name="volume_off" />
					<span className="tooltip">Unmute</span>
				</a>
			)
		} else {
			return (
				<a className="control mute-control has-tooltip" onClick={() => this.props.onMuteChange(true)}>
					<Icon className="muted" name="volume_mute" />
					<span className="tooltip">Mute</span>
				</a>
			)
		}
	}

	render(){
		var className = "volume-control";
		if (this.props.mute){
			className += " muted";
		}
		if (this.props.className){
			className += " "+this.props.className;
		}

		return (
			<span className={className}>
				{this.props.NoMuteButton ? null : this.renderMuteButton()}
				<div className="slider-wrapper">
					<div className={"slider horizontal"+(this.props.mute ? " disabled" : "")}>
						<input 
							type="range" 
							min="0" 
							max="25" 
							value={this.props.volume/4}
							onChange={e => this.handleChange(parseInt(e.target.value)*4)}
						/>
						<div className="track">
							<div className="progress" style={{ width: this.props.volume+'%' }}></div>
						</div>
					</div>
				</div>
			</span>
		);
	}
}

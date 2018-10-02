
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

	render(){
		return (
			<div className={"slider__wrapper "+(this.props.className ? this.props.className : "")}>
				<div className={"slider slider--volume "+(this.props.mute ? "slider--muted" : "")}>
					<input 
						className="slider__input"
						type="range" 
						min="0" 
						max="25" 
						value={this.props.volume/4}
						onChange={e => this.handleChange(parseInt(e.target.value)*4)}
					/>
					<div className="slider__track">
						<div className="slider__track__progress" style={{ width: this.props.volume+'%' }}></div>
					</div>
				</div>
			</div>
		);
	}
}

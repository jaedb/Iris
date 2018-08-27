
import React, { PropTypes } from 'react';

import * as helpers from '../../helpers';

export default class LatencyControl extends React.Component{

	constructor(props){
		super(props);
		
		this.handleChange = helpers.throttle(this.handleChange.bind(this), 100);
	}

	handleChange(value){
		this.props.onChange(value, this.props.value);
	}

	render(){

		// Zero, or positive value
		if (this.props.value >= 0){
			var value = this.props.value;
			if (value > this.props.max){
				value = this.props.max;
			}
			var percentage = Math.round((value / this.props.max) * 100 / 2);
			var left = 50;
			var width = percentage;
			var negative = false;

			if (width > 50) width = 50;

		// Negative value
		// We reverse it to a positive for easier maths and style rules
		} else {
			var value = -this.props.value;
			if (value < -this.props.max){
				value = -this.props.max;
			}
			var percentage = Math.round((value / this.props.max) * 100 / 2);
			var left = 50 - percentage;
			var width = percentage;
			var negative = true;

			if (left < 0) left = 0;
			if (width > 50) width = 50;
		}

		return (
			<span className="latency-control">
				<div className="slider-wrapper">
					<div className="slider horizontal">
						<input 
							type="range" 
							min="-100" 
							max="100" 
							value={this.props.value}
							onChange={e => this.handleChange(parseInt(e.target.value))}
						/>
						<div className="zero"></div>
						<div className="track">
							<div className={"progress "+(negative ? 'negative' : 'positive')} style={{ width: width+'%', left: left+'%' }}></div>
						</div>
					</div>
				</div>
			</span>
		);
	}
}


import React, { PropTypes } from 'react';

export default class LatencyControl extends React.Component{

	constructor(props){
		super(props);
	}

	handleClick(e){
		var slider = e.target;
		if (slider.className != 'slider'){
			slider = slider.parentElement;
		}

		var sliderX = e.clientX - slider.getBoundingClientRect().left;
		var sliderWidth = slider.getBoundingClientRect().width;

		// Clicked left half
		if (sliderX < (sliderWidth/2)){
			var percent = Math.round((sliderX / (sliderWidth/2) ) * 100);

			// Invert our percentage
			percent = 100 - percent;

			var value = -(this.props.max * (percent / 100));

			// Handle maximum value limits
			if (value < -this.props.max){
				value = -this.props.max;
			}

		// Second half
		} else {

			// Subtract half the slider's width from our click position
			sliderX = sliderX - (sliderWidth/2);
			var percent = Math.round((sliderX / (sliderWidth/2) ) * 100);

			var value = this.props.max * (percent / 100);

			// Handle maximum value limits
			if (value > this.props.max){
				value = this.props.max;
			}
		}

		this.props.onChange(value);
	}

	handleWheel(e){

		// Identify which direction we've scrolled (inverted)
		// This is simplified and doesn't consider momentum as it varies wildly
		// between browsers and devices
		var direction = (e.deltaY > 0 ? -1 : 1)
		var value = this.props.value;

		value += direction * 5

		if (value > this.props.max){
			value = this.props.max;
		} else if (value < -this.props.max){
			value = -this.props.max;
		}

		this.props.onChange(value);
		e.preventDefault();
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
		}

		return (
			<span className="latency-control" onWheel={e => this.handleWheel(e)}>
				<div className="slider-wrapper">
					<div className="slider horizontal" onClick={e => this.handleClick(e)}>
						<div className="track">
							<div className="progress" style={{ width: width+'%', left: left+'%' }}></div>
						</div>
					</div>
				</div>
			</span>
		);
	}
}

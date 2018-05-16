
import React, { PropTypes } from 'react'
import Icon from '../Icon'

export default class VolumeControl extends React.Component{

	constructor(props){
		super(props)
	}

	handleClick(e){
		var old_percent = this.props.volume;
		var slider = e.target;
		if (slider.className != 'slider' ) slider = slider.parentElement;

		var sliderX = e.clientX - slider.getBoundingClientRect().left;
		var sliderWidth = slider.getBoundingClientRect().width;
		var percent = Math.round((sliderX / sliderWidth ) * 100);

		if (percent > 100){
			percent = 100
		} else if (percent < 0){
			percent = 0
		}

		this.props.onVolumeChange(percent, old_percent);
	}

	handleWheel(e){
		if (this.props.scrollWheel){
			var old_percent = this.props.volume;
			
			// Identify which direction we've scrolled (inverted)
			// This is simplified and doesn't consider momentum as it varies wildly
			// between browsers and devices
			var direction = (e.deltaY > 0 ? -1 : 1)
			var percent = this.props.volume;

			percent += direction * 5

			if (percent > 100){
				percent = 100
			} else if (percent < 0){
				percent = 0
			}

			this.props.onVolumeChange(percent, old_percent);
			e.preventDefault();
		}
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
			<span className={className} onWheel={e => this.handleWheel(e)}>
				{this.renderMuteButton()}
				<div className="slider-wrapper">
					<div className="slider horizontal" onClick={e => this.handleClick(e)}>
						<div className="track">
							<div className="progress" style={{ width: this.props.volume+'%' }}></div>
						</div>
					</div>
				</div>
			</span>
		);
	}
}

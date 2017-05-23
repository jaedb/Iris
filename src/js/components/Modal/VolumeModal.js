
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

import Icon from '../Icon'
import * as helpers from '../../helpers'

export default class VolumeModal extends React.Component{
	constructor(props){
		super(props)
	}

	changeVolume(e){
		var slider = e.target;

		var sliderY = e.clientY - slider.getBoundingClientRect().top;
		var sliderHeight = slider.getBoundingClientRect().height;
		var percent = 100 - parseInt( sliderY / sliderHeight * 100 );

		this.props.mopidyActions.setVolume( percent )
	}

	toggleMute(e){
		this.props.mopidyActions.setMute(!this.props.mute)
	}

	render(){
		return (
			<div>
				<a className="toggle-mute" onClick={e => this.toggleMute(e)}>
					{this.props.mute ? <FontAwesome className="muted" name="volume-off" /> : <FontAwesome name="volume-down" />}
				</a>
				<div className={this.props.mute ? "slider vertical disabled" : "slider vertical"} onClick={e => this.changeVolume(e)}>
					<div className="track">
						<div className="progress" style={{ height: this.props.volume+'%' }}></div>
					</div>
				</div>
			</div>
		)
	}
}
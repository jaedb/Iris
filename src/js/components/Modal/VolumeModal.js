
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

import Icon from '../Icon'
import * as helpers from '../../helpers'

export default class VolumeModal extends React.Component{
	constructor(props){
		super(props)
	}

	handleClick(e){
		var slider = e.target;

		var sliderY = e.clientY - slider.getBoundingClientRect().top;
		var sliderHeight = slider.getBoundingClientRect().height;
		var percent = 100 - parseInt( sliderY / sliderHeight * 100 );

		this.props.mopidyActions.setVolume( percent )
	}

	render(){
		return (
			<div>
				<div className="slider-wrapper">
					<div className="slider vertical" onClick={ (e) => this.handleClick(e) } >
						<div className="track">
							<div className="progress" style={{ height: this.props.volume+'%' }}></div>
						</div>
					</div>
				</div>
			</div>
		)
	}
}
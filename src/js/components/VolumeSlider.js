
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

export default class VolumeSlider extends React.Component{

	constructor(props) {
		super(props);
	}

	handleChange(e){
		this.props.onChange( parseInt(e.target.value) )
	}

	render(){
		return (
			<input
				type="range"
				value={this.props.volume}
				min="0"
				max="100"
				onChange={ (e) => this.handleChange(e) } />
		);
	}
}
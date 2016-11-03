
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

export default class VolumeSlider extends React.Component{

	constructor(props) {
		super(props);
		this.state = {
			volume: 0
		}
	}

	componentWillReceiveProps( nextProps ){
		this.setState({ volume: nextProps.volume })
	}

	handleClick(e){
		var slider = e.target;
		if( slider.className != 'slider' ) slider = slider.parentElement;

		var sliderX = e.clientX - slider.getBoundingClientRect().left;
		var sliderWidth = slider.getBoundingClientRect().width;
		var percent = parseInt( sliderX / sliderWidth * 100 );
		
		if( this.props.volume != percent ){
			this.props.onChange( volume )
		}
	} 

	render(){
		return (
			<div className="slider" onClick={ (e) => this.handleClick(e) } >
				<div className="progress" style={{ width: this.props.volume+'%' }}></div>
			</div>
		);
	}
}
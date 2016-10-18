
import React, { PropTypes } from 'react'

export default class Thumbnail extends React.Component{

	constructor(props) {
		super(props);
	}

	sizedImageURL(){
		var images = this.props.images;
		switch( this.props.size ){
			case 'small':
				break;

			default:
				return images[images.length-1].url
		}
	}

	render(){
		return (
			<img src={ this.sizedImageURL() } />
		);
	}
}
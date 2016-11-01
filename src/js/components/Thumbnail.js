
import React, { PropTypes } from 'react'
import * as helpers from '../helpers'

export default class Thumbnail extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			small: false,
			medium: false,
			large: false,
			huge: false
		}
	}

	componentDidMount(){
		this.mapImageSizes();
	}

	componentWillReceiveProps( nextProps ){
		this.mapImageSizes( nextProps.images );
	}

	mapImageSizes( images = this.props.images ){
		var state = this.state;

		if( images.length <= 0 ){
			state = {
				small: require('../../images/no-image.svg'),
				medium: require('../../images/no-image.svg'),
				large: require('../../images/no-image.svg'),
				huge: require('../../images/no-image.svg')
			}
		}else{
			state = helpers.sizedImages( images );
		}

		this.setState( state );
	}

	render(){
		var style = {
			backgroundImage: 'url("'+this.state[this.props.size]+'")'
		}
		var className = 'thumbnail '+this.props.size;
		if( this.props.circle ) className += ' circle';
		
		return (
			<div className={className}>
				<div className="image" style={style}></div>
			</div>
		);
	}
}
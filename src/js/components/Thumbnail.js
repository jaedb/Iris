
import React, { PropTypes } from 'react'
import * as helpers from '../helpers'

export default class Thumbnail extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			url: require('../../assets/no-image.svg')
		}
	}

	componentDidMount(){
		this.mapImageSizes();
	}

	componentWillReceiveProps( nextProps ){
		this.mapImageSizes( nextProps );
	}

	mapImageSizes( props = this.props ){		

		// single image
		if( this.props.image ){
			this.setState({ url: this.props.image })

		// multiple images
		}else if( this.props.images && this.props.images.length > 0 ){
			var images = helpers.sizedImages( this.props.images )
			var size = 'medium'
			if( this.props.size ) size = this.props.size
			this.setState({ url: images[size] })
		}
	}

	render(){
		var style = {
			backgroundImage: 'url("'+this.state.url+'")'
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
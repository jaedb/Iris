
import React, { PropTypes } from 'react'
import * as helpers from '../helpers'

export default class Thumbnail extends React.Component{

	constructor(props) {
		super(props);
	}

	// TODO: ascertain whether this is improving or hindering performance
	// The UI appears to work perfectly fine without this
	shouldComponentUpdate(nextProps, nextState){

		// no images at all, and we already know it
		if( 
			typeof(nextProps.image) == 'undefined' && 
			typeof(this.props.image) == 'undefined' &&
			typeof(nextProps.images) == 'undefined' && 
			typeof(this.props.images) == 'undefined'
			) return false

		// image changed
		if( !this.props.image && nextProps.image ) return true
		if( this.props.image && nextProps.image ) return true
		if( this.props.image != nextProps.image ) return true

		// images array changed
		if( typeof(this.props.images) === 'undefined' && nextProps.images ) return true
		if( this.props.images && typeof(nextProps.images) === 'undefined' ) return true
		if( this.props.images.length != nextProps.images.length ) return true

		// image item changed	
		var size = 'medium'
		var images = helpers.sizedImages( nextProps.images )
		if( this.props.size ) size = this.props.size
		if( this.props.images[size] != images[size] ) return true

		return false
	}

	mapImageSizes( props = this.props ){

		// no images
		if( !this.props.image && !this.props.images ){
			return require('../../assets/no-image.svg')

		// single image
		}else if( this.props.image ){
			return this.props.image

		// multiple images
		}else if( this.props.images && this.props.images.length > 0 ){
			var images = helpers.sizedImages( this.props.images )
			var size = 'medium'
			if( this.props.size ) size = this.props.size
			return images[size]
		}
	}

	render(){
		var image = this.mapImageSizes()
		var style = { backgroundImage: 'url("'+image+'")' }
		var className = 'thumbnail '+this.props.size;
		if( this.props.circle ) className += ' circle';
		
		return (
			<div className={className}>
				<div className="image" style={style}></div>
			</div>
		);
	}
}
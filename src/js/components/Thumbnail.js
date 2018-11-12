
import React, { PropTypes } from 'react';
import { connect } from 'react-redux';
import { createStore, bindActionCreators } from 'redux';
import Link from './Link';

import Icon from './Icon'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'

class Thumbnail extends React.Component{

	constructor(props){
		super(props);
	}

	mapImageSizes(){

		// Single image
		if (this.props.image){
			return this.props.image;

		// Multiple images
		} else if (this.props.images){
			var images = this.props.images;

			// An array of image objects (eg Artists), so just pick the first one
			if (Array.isArray(images) && images.length > 0){
				images = images[0];
			}

			// Default to medium-sized image, but accept size property as override
			var size = 'medium';
			if (this.props.size){
				size = this.props.size;
			}

			// Return the requested size
			if (images[size]){
				return images[size];
			}
		}

		// No images
		return null;
	}

	render(){
		var image = this.mapImageSizes();
		var class_name = 'thumbnail ';
		if (this.props.size){
			class_name += ' '+this.props.size;
		}
		if (this.props.circle){
			class_name += ' circle';
		}
		if (this.props.className){
			class_name += ' '+this.props.className;
		}
		
		var zoom_icon = null;
		if (this.props.canZoom && image){
			zoom_icon = <Link className="zoom" to={global.baseURL+'image-zoom?url='+image}><Icon name="search" /></Link>;
		}

		return (
			<div className={class_name}>
				<div className="image loaded" style={{backgroundImage: 'url("'+(image ? image : require('../../assets/no-image.svg'))+'")'}}></div>
				{zoom_icon}
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Thumbnail)
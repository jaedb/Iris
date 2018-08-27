
import React, { PropTypes } from 'react';
import { connect } from 'react-redux';
import { createStore, bindActionCreators } from 'redux';
import { Link } from 'react-router';

import Icon from './Icon'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'

class Thumbnail extends React.Component{

	constructor(props){
		super(props);
	}
/*
	// TODO: ascertain whether this is improving or hindering performance
	// The UI appears to work perfectly fine without this
	shouldComponentUpdate(nextProps, nextState){

		// no images at all, and we already know it
		if (!nextProps.image && !this.props.image && !nextProps.images && !this.props.images) return false

		// image changed
		if (!this.props.image && nextProps.image) return true
		if (this.props.image && nextProps.image) return true
		if (this.props.image != nextProps.image) return true

		// images array changed
		if (this.props.images === undefined && nextProps.images ) return true
		if (this.props.images && nextProps.images === undefined) return true
		if (this.props.images && !nextProps.images || this.props.images.length != nextProps.images.length ) return true

		// image item changed	
		var size = 'medium'
		var images = helpers.sizedImages(nextProps.images )
		if (this.props.size ) size = this.props.size
		if (this.props.images[size] != images[size]) return true

		return false
	}
	*/

	mapImageSizes(props = this.props){

		// Single image
		if (this.props.image){
			return this.props.image;

		// Multiple images
		} else if (this.props.images){
			var images = helpers.sizedImages(this.props.images);

			// Default to medium-sized image, but accept size property as override
			var size = 'medium';
			if (this.props.size){
				size = this.props.size;
			}

			return images[size];
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

import React from 'react'

import * as helpers from '../helpers';

export default class Parallax extends React.Component{

	constructor(props){
		super(props);

		this.state = {
			loaded: false,
			url: null
		}
	}

	componentWillMount(){
		if (this.props.image){
			this.loadImage(this.props.image);
		}
	}

	componentWillReceiveProps(nextProps){
		if (nextProps.image != this.state.url){
			this.loadImage(nextProps.image);
		}
	}

	loadImage(url){
		if (url && url !== ""){

			this.setState({
				loaded: helpers.isCached(url),
				url: url
			});

			var self = this;
			var imageObject = new Image();
			imageObject.src = url;

			imageObject.onload = function(){
				self.setState({
					loaded: true,
					url: url
				});
			}

		// No Image, so reset it
		} else {
			this.setState({
				loaded: false,
				url: null
			});
		}
	}

	render(){
		var class_name = "parallax preserve-3d";
		if (this.props.blur){
			class_name += " parallax--blur";
		}
		if (this.state.loaded){
			class_name += " parallax--loaded";
		}
		if (this.props.fixedHeight){
			class_name += " parallax--fixed-height";
		} else {
			class_name += " parallax--flexible-height";
		}

		var style = {};
		if (this.state.loaded && this.state.url){
			style = {backgroundImage: 'url('+this.state.url+')'};
		}

		return (
			<div className={class_name}>
				<div className="parallax__layer preserve-3d">
					<div className="parallax__image" style={style}></div>
					<div className="parallax__overlay"></div>
				</div>
			</div>
		);
	}
}




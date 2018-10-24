
import React, { PropTypes } from 'react'

export default class Parallax extends React.Component{

	constructor(props){
		super(props);

		this.state = {
			loaded: false,
			url: ''
		}
	}

	componentWillMount(){
		if (this.props.image){
			this.loadImage(this.props.image);
		}
	}

	componentWillReceiveProps(nextProps){
		if ((!this.state.url || nextProps.image != this.state.url ) && nextProps.image){
			this.loadImage(nextProps.image);
		}
	}

	loadImage(url){
		var self = this;
		var imageObject = new Image();
		imageObject.src = url;

		imageObject.onload = function(){
			self.setState({
				loaded: true,
				url: url
			});
		}
	}

	render(){
		var class_name = "parallax";
		if (this.props.blur){
			class_name += " parallax--blur";
		}
		if (this.state.loaded){
			class_name += " parallax--loaded";
		}

		return (
			<div className={class_name}>
				<div className="parallax__image" style={{backgroundImage: 'url('+this.state.url+')'}}></div>
				<div className="parallax__overlay"></div>
			</div>
		);
	}
}




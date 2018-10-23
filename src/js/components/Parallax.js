
import React, { PropTypes } from 'react'

export default class Parallax extends React.Component{

	constructor(props){
		super(props);
	}

	render(){
		var class_name = "parallax";
		if (this.props.blur){
			class_name += " parallax--blur";
		}

		return (
			<div className={class_name}>
				<div className="parallax__image" style={{backgroundImage: 'url('+this.props.image+')'}}></div>
				<div className="parallax__overlay"></div>
			</div>
		);
	}
}




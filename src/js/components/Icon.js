
import React, { PropTypes } from 'react';

export default class Icon extends React.Component{

	constructor(props){
		super(props);
	}

	render(){
		var className = 'icon icon-'+this.props.name;
		if (this.props.className){
			className += ' '+this.props.className;
		}

		return <i className={className}></i>;
	}
}
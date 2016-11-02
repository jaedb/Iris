
import React, { PropTypes } from 'react'
import { Link } from 'react-router'

export default class Icon extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		var className = 'icon';
		if( this.props.className ) className += ' '+this.props.className;
		var src = require('../../assets/icons/'+this.props.name+'.svg');
		return <img className={className} src={src} />;
	}
}
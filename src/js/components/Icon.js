
import React, { PropTypes } from 'react'
import { Link } from 'react-router'

export default class Icon extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		var src = require('../../images/icons/'+this.props.name+'.svg');
		return <img className="icon" src={src} />;
	}
}
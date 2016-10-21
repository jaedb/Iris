
import React, { PropTypes } from 'react'

export default class extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		var uriElements = this.props.uri.split(':');
		if( uriElements.length <= 0 ) return false;
		return <span>{ uriElements[0] }</span>;
	}
}
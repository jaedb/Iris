
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'
import { browserHistory } from 'react-router'

export default class ListItem extends React.Component{

	constructor(props) {
		super(props);
	}

	handleClick(e){
		if( e.target.tagName.toLowerCase() !== 'a' ){
			browserHistory.push( this.props.link );
		}
	}

	render(){
		var className = 'list-item';
		if( this.props.extraClasses ) className += ' '+this.props.extraClasses;

		return (
			<li className={className} onClick={ (e) => this.handleClick(e) }>
				{ this.props.children }
			</li>
		);
	}
}
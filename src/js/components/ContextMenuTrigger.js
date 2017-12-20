
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

export default class ContextMenuTrigger extends React.Component{

	constructor(props){
		super(props);
	}

	handleClick(e){
		e.preventDefault();
		e.stopPropagation();
		this.props.onTrigger(e);
	}

	render(){
		var className = 'context-menu-trigger mouse-contextable touch-contextable'
		if (this.props.className){
			className += ' '+this.props.className
		}
		return (
			<span
				className={className}
				onClick={e => this.handleClick(e)}>
					<span className="dot"></span>
					<span className="dot"></span>
					<span className="dot"></span>
			</span>
		);
	}
}


import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

export default class ContextMenuTrigger extends React.Component{

	constructor(props){
		super(props);
	}

	render(){
		var className = 'context-menu-trigger'
		if (this.props.className){
			className += ' '+this.props.className
		}
		return (
			<span className={className}
				onMouseDown={e => this.props.onTrigger(e)}
				onTouchEnd={e => this.props.onTrigger(e)}>
					<span className="dot"></span>
					<span className="dot"></span>
					<span className="dot"></span>
			</span>
		);
	}
}

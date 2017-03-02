
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

export default class ContextMenuTrigger extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		var className = 'context-menu-trigger'
		if (this.props.className){
			className += ' '+this.props.className
		}
		return (
			<button
				className={className}
				onClick={e => this.props.onTrigger(e)}>
				<FontAwesome name="ellipsis-v" />
			</button>
		);
	}
}


import React, { PropTypes } from 'react'
import Icon from './Icon'
import SidebarToggleButton from './SidebarToggleButton'

export default class Header extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		return (
			<header>
				<Icon name={ this.props.icon } />
				<SidebarToggleButton />
				<h1>{ this.props.title }</h1>
				{ this.props.actions ? <div className="actions">{ this.props.actions }</div> : null }
			</header>
		);
	}
}
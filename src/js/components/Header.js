
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

import Icon from './Icon'
import SidebarToggleButton from './SidebarToggleButton'

export default class Header extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			expanded: false
		}
	}

	renderActions(){
		if (!this.props.actions) return null

		return (
			<div className={this.state.expanded ? 'expanded actions' : 'actions'}>
				<button className="trigger" onClick={e => this.setState({expanded: !this.state.expanded})}>
					<FontAwesome name="ellipsis-v" />
				</button>
				<span className="items">
					<span className="liner">
						{ this.props.actions }
					</span>
				</span>
			</div>
		)
	}

	render(){
		return (
			<header>
				<Icon name={ this.props.icon } />
				<SidebarToggleButton />
				<h1>{ this.props.title }</h1>
				{this.renderActions()}
			</header>
		);
	}
}
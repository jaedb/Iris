
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

import Icon from './Icon'
import SearchForm from './SearchForm'
import SidebarToggleButton from './SidebarToggleButton'
import ContextMenuTrigger from './ContextMenuTrigger'

export default class Header extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			expanded: false
		}
	}

	handleContextMenuTrigger(e,options){		
		e.preventDefault()
		var data = {
			e: e,
			context: 'custom',
			title: this.props.title,
			options: options
		}
		this.props.uiActions.showContextMenu(data)
	}

	renderOptions(){
		if (!this.props.options) return null

		return (
			<div className='options'>
				<ContextMenuTrigger onTrigger={e => this.handleContextMenuTrigger(e,this.props.options)} />
				<span className="items">
					<span className="liner">
						{ this.props.options }
					</span>
				</span>
			</div>
		)
	}

	render(){
		if (this.props.search){
			return (
				<header className={(this.props.className ? this.props.className : null)}>
					<Icon name="search" />
					<SidebarToggleButton />
					<SearchForm />
					{this.renderOptions()}
				</header>
			)
		} else {
			return (
				<header className={(this.props.className ? this.props.className : null)}>
					<Icon name={this.props.icon} />
					<SidebarToggleButton />
					<h2>{ this.props.title }</h2>
					{this.renderOptions()}
				</header>
			)
		}
	}
}
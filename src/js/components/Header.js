
import React, { PropTypes } from 'react'

import Icon from './Icon';
import ContextMenuTrigger from './ContextMenuTrigger';

export default class Header extends React.Component{

	constructor(props){
		super(props);

		this.state = {
			expanded: false
		}
	}

	handleContextMenuTrigger(e,options){

		// We have an override trigger (eg Album, Playlist)
		if (this.props.handleContextMenuTrigger){
			return this.props.handleContextMenuTrigger(e)
		} else {
			e.preventDefault()
			var data = {
				e: e,
				context: 'custom',
				title: this.props.title,
				options: options
			}
			this.props.uiActions.showContextMenu(data)
		}
	}

	renderContextMenuTrigger(){

		// No custom trigger, nor any options
		if (!this.props.handleContextMenuTrigger && !this.props.options){
			return null;
		}

		return <ContextMenuTrigger onTrigger={e => this.handleContextMenuTrigger(e,this.props.options)} />
	}

	renderOptions(){
		if (!this.props.options && !this.props.handleContextMenuTrigger){
			return null;
		}

		return (
			<div className='options'>
				{this.renderContextMenuTrigger()}
				<span className="items">
					<span className="liner">
						{this.props.options ? this.props.options : null}
					</span>
				</span>
			</div>
		)
	}

	render(){
		return (
			<header className={(this.props.className ? this.props.className : null)}>
				<h1>
					{this.props.children ? this.props.children : null}
				</h1>
				{this.renderOptions()}
			</header>
		)
	}
}
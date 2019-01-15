
import React from 'react';
import { Link, NavLink } from 'react-router-dom';

import * as helpers from '../helpers';

/**
 * Extends react-router's Link but provides the ability to hook in to the navigation event
 * which lets us scroll to the top of our <main> for a more traditional navigation experience
 **/
export default class extends React.Component{

	constructor(props){
		super(props);
	}

	handleClick(e){
		if (!this.props.retainScroll){
			helpers.scrollTo(this.props.scrollTo, (this.props.scrollTo));
		}
	}

	render(){
		var className = null;
		if (this.props.className){
			var className = this.props.className;
		} else {
			var className = "";
		}

		if (this.props.nav){
			return (
				<NavLink 
					onClick={e => this.handleClick(e)}
					activeClassName="active"
					className={className}
					to={this.props.to}>
						{this.props.children}
				</NavLink>
			);
		} else {
			return (
				<Link 
					onClick={e => this.handleClick(e)}
					className={className}
					to={this.props.to}>
						{this.props.children}
				</Link>
			);
		}
	}
}
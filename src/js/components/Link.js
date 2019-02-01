
import React from 'react';
import { Link } from 'react-router-dom';

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

	isLinkActive(link){
		if (this.props.exact){
			return this.props.history.location.pathname === link;
		} else {
			return this.props.history.location.pathname.startsWith(link);
		}
	}

	render(){
		var className = "";
		if (this.props.className){
			className += this.props.className;
		}

		// We have an active detector method
		// This is used almost solely by the Sidebar navigation
		if (this.props.history !== undefined){
			if (this.isLinkActive(this.props.to)){
				if (this.props.activeClassName){
					className += " "+this.props.activeClassName;
				} else {
					className += " active";
				}
			}
		}

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
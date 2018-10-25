
import React from 'react';
import {Link} from 'react-router';

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
			if (this.props.scrollTo){
				var element = document.getElementById(this.props.scrollTo);
				if (element){
					element.scrollIntoView();
				}
			} else {
				helpers.scrollToTop();
			}
		}
	}

	render(){
		return (
			<Link 
				onClick={e => this.handleClick(e)}
				className={this.props.className ? this.props.className : null}
				activeClassName={this.props.activeClassName}
				to={this.props.to}>
					{this.props.children}
			</Link>
		);
	}
}
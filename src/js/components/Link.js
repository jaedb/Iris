
import React from 'react';
import {Link} from 'react-router-dom';

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
		return (
			<Link 
				onClick={e => this.handleClick(e)}
				className={this.props.className ? this.props.className : null}
				to={this.props.to}>
					{this.props.children}
			</Link>
		);
	}
}
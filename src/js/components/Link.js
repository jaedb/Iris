
import React from 'react';
import { Link } from 'react-router';

export default class extends React.Component{

	constructor(props){
		super(props);
	}

	handleClick(e){
		if (!this.props.retainScroll){
			document.getElementById('main').scrollTo(0, 0);
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

import React, { PropTypes } from 'react';
import FontAwesome from 'react-fontawesome';

export default class Icon extends React.Component{

	constructor(props){
		super(props);
	}

	handleClick(e){
		if (this.props.onClick){
			this.props.onClick(e);
		}
	}

	render(){
		var className = "icon";
		if (this.props.className){
			className += ' '+this.props.className;
		}

		switch (this.props.type){
			case 'fontawesome':	
				return <FontAwesome type="fontawesome" name={this.props.name} onClick={e => this.handleClick(e)} />;

			default:
				className += ' material-icon';		
				return <i className={className} onClick={e => this.handleClick(e)}>{this.props.name}</i>;
		}
	}
}

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
		var className = "icon icon--"+(this.props.type ? this.props.type : 'material');
		if (this.props.className){
			className += ' '+this.props.className;
		}

		switch (this.props.type){
			case 'svg':	
				var src = require('../../assets/icons/'+this.props.name+'.svg');
				return <img className={className} src={src} onClick={e => this.handleClick(e)} />;

			case 'gif':	
				var src = require('../../assets/icons/'+this.props.name+'.gif');
				return <img className={className} src={src} onClick={e => this.handleClick(e)} />;

			case 'fontawesome':	
				return <FontAwesome className={className} type="fontawesome" name={this.props.name} onClick={e => this.handleClick(e)} />;

			default:
				return <i className={className} onClick={e => this.handleClick(e)}>{this.props.name}</i>;
		}
	}
}
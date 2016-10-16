
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

export default class ConfirmationButton extends React.Component{

	constructor(props) {
		super(props);
		this.state = {
			confirming: false
		}
	}

	handleClick(e){
		if( this.state.confirming ){
			this.setState({ confirming: false });
			this.props.onConfirm();
		}else{
			this.setState({ confirming: true });
		}
	}

	render(){

		var className = 'button';
		var content = this.props.content;

		if( this.state.confirming ){
			className += ' confirming';	
			content = this.props.confirmingContent;
		}

		return (
			<span className={className} onClick={ (e) => this.handleClick(e) }>
				{ content }
			</span>
		);
	}
}
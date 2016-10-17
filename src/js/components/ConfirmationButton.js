
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

export default class ConfirmationButton extends React.Component{

	constructor(props) {
		super(props);
		this.state = {
			confirming: false
		}
		this.confirming = false;
		this.unconfirmTimer = false;
	}

	handleClick(e){
		if( this.state.confirming ){
			this.setState({ confirming: false });
			this.props.onConfirm();
		}else{
			this.setState({ confirming: true });
		}
	}

	handleMouseEnter(e){
		clearTimeout( this.unconfirmTimer );
	}

	handleMouseLeave(e){
		this.unconfirmTimer = setTimeout(
			function(){
				this.setState({ confirming: false });
			}.bind(this),
			1500
		);
	}

	render(){

		var className = 'button';
		var content = this.props.content;

		if( this.state.confirming ){
			className += ' confirming';	
			content = this.props.confirmingContent;
		}

		return (
			<button
				className={className}
				onClick={ (e) => this.handleClick(e) }
				onMouseLeave={ (e) => this.handleMouseLeave(e) }
				onMouseEnter={ (e) => this.handleMouseEnter(e) }>
					{ content }
			</button>
		);
	}
}
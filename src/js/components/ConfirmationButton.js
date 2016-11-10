
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

export default class ConfirmationButton extends React.Component{

	constructor(props) {
		super(props);
		this.state = {
			timing_out: false,
			confirming: false
		}
		this.confirming = false;
		this.unconfirmTimer = false;
	}

	componentWillUnmount(){
		clearTimeout( this.unconfirmTimer );
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
		this.setState({ timing_out: false });
		clearTimeout( this.unconfirmTimer );
	}

	handleMouseLeave(e){
		if( this.state.confirming ){
			this.setState({ timing_out: true });
			this.unconfirmTimer = setTimeout(
				function(){
					this.setState({ confirming: false });
				}.bind(this),
				2000
			);
		}
	}

	render(){

		var className = 'button';
		var content = this.props.content;

		if( this.state.confirming ){
			className += ' confirming';	
			content = this.props.confirmingContent;
			if( this.state.timing_out ) className += ' timing-out';
		}

		if( this.props.className ) className += ' '+this.props.className

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

import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import * as helpers from '../../helpers'
import Icon from '../../components/Icon'

class Modal extends React.Component{

	constructor(props){
		super(props);
		this.handleKeyUp = this.handleKeyUp.bind(this);
	}

	componentWillMount(){
		$('body').addClass('modal-open');
		window.addEventListener("keyup", this.handleKeyUp, false);
	}

	componentWillUnmount(){
		$('body').removeClass('modal-open');
		window.removeEventListener("keyup", this.handleKeyUp, false);
	}

	handleKeyUp(e){
		if (this.props.shortkeys_enabled && e.keyCode == 27){ // esc
			window.history.back();
		}
	}

	render(){

		var className = "modal";
		if (this.props.className){
			className += " "+this.props.className;
		}

		return (
			<div className={className}>

				<div className="controls">
					{this.props.noclose ? null : <div className="control close" onClick={e => window.history.back()}>
						<Icon name="close" className="white" />
					</div> }
				</div>

				<div className="content">
					{this.props.children}
				</div>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		shortkeys_enabled: state.ui.shortkeys_enabled
	}
}

const mapDispatchToProps = (dispatch) => {
	return {}
}

export default connect(mapStateToProps, mapDispatchToProps)(Modal)
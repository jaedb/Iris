
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Icon from '../Icon'

import * as helpers from '../../helpers';

class SnapcastPowerButton extends React.Component{

	constructor(props){
		super(props);

		this.handleClick = this.handleClick.bind(this);
	}

	handleClick(e){
		var command = JSON.parse(this.props.client.commands.power_on);
		$.ajax(command);
	}

	render(){
		var classname = "snapcast-power-button";
		if (this.props.className){
			classname += " "+this.props.className;
		}
		return (
			<span className={classname} onClick={e => this.handleClick(e)}>
				<Icon name="power_settings_new" />
			</span>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {}
}

const mapDispatchToProps = (dispatch) => {
	return {}
}

export default connect(mapStateToProps, mapDispatchToProps)(SnapcastPowerButton)
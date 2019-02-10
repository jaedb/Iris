
import React from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Icon from '../Icon'

import * as helpers from '../../helpers';
import * as snapcastActions from '../../services/snapcast/actions';

export default class SnapcastPowerButton extends React.Component{

	constructor(props){
		super(props);

		this.handleClick = this.handleClick.bind(this);
	}

	handleClick(e){
		this.props.onClick(e);
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

import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import * as uiActions from '../services/ui/actions'

class SidebarToggleButton extends React.Component{

	constructor(props) {
		super(props);
	}

	handleClick(e){
		this.props.uiActions.toggleSidebar()
	} 

	render(){

		var classname = 'sidebar-toggle'
		if( this.props.open ) classname += ' open'
		if( this.props.className ) classname += ' '+this.props.className

		return (
			<div className={classname} onClick={ () => this.handleClick() }>
				<FontAwesome className="open" name="bars" /> 
				<FontAwesome className="close" name="chevron-left" />
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		open: state.ui.sidebar_open
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(SidebarToggleButton)
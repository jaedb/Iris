
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { createStore, bindActionCreators } from 'redux'

import * as uiActions from '../../services/ui/actions'
import * as helpers from '../../helpers'

import Icon from '../Icon'

class ImageZoomModal extends React.Component{

	constructor(props){
		super(props)
	}

	handleClick(e, connectionid){		
		e.preventDefault()
		this.props.uiActions.closeModal()
		return false;
	}

	render(){
		return (
			<div className="image-zoom-modal">
				{this.props.data.url ? <img src={this.props.data.url} /> : null }
			</div>
		)
	}
}

const mapStateToProps = (state, ownProps) => {
	return {}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(ImageZoomModal)
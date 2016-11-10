
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Icon from './Icon'

export default class Dropzone extends React.Component{

	constructor(props) {
		super(props);
	}

	handleMouseUp(e){
		// TODO: sanity check if this is a valid dropzone

		return this.props.handleMouseUp(e)
	}

	render(){
		if( !this.props.data ) return null

		return (
			<div className="dropzone" onMouseUp={ e => this.handleMouseUp(e) }>
				<Icon className="white" name={ this.props.data.icon } />
				<span className="title">{ this.props.data.title }</span>
			</div>
		);
	}
}
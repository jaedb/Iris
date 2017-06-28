
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Icon from './Icon'

export default class Dropzone extends React.Component{

	constructor(props) {
		super(props)

		this.state = {
			hover: false
		}

		this.handleMouseOver = this.handleMouseOver.bind(this)
		this.handleMouseOut = this.handleMouseOut.bind(this)
	}

	componentWillMount(){
		window.addEventListener("mouseover", this.handleMouseOver, false);
		window.addEventListener("mouseout", this.handleMouseOut, false);
	}

	componentWillUnmount(){
		window.removeEventListener("mouseover", this.handleMouseOver, false);
		window.removeEventListener("mouseout", this.handleMouseOut, false);
	}

	handleMouseOver(e){
		this.setState({hover: true})
	}

	handleMouseOut(e){
		this.setState({hover: false})
	}

	render(){
		if( !this.props.data ) return null

		return (
			<div className={this.state.hover?"dropzone hover":"dropzone"} onMouseUp={ e => this.props.handleMouseUp(e) }>
				<Icon className="white" name={ this.props.data.icon } />
				<span className="title">{ this.props.data.title }</span>
			</div>
		);
	}
}
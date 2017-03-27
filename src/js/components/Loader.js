
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

export default class Loader extends React.Component{

	constructor(props) {
		super(props)
	}

	render(){
		if (!this.props.load_queue){
			return null
		}

		var load_queue = this.props.load_queue
		var is_loading = false
		for (var key in load_queue){
			if (load_queue.hasOwnProperty(key)){
				is_loading = true
				break
			}
		}

		if (is_loading){
			return (
				<div className="loader active"></div>
			)
		} else {	
			return (
				<div className="loader"></div>
			)
		}
	}
}
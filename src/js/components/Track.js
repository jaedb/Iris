
import React, { PropTypes } from 'react'

export default class Track extends React.Component{

	constructor(props) {
		super(props);
	}

	handleClick( e ){
		console.log('Track clicked', this.props.track.name)
	}

	render(){
		return <div className="track" onClick={(e) => this.handleClick(e)}>#{this.props.track.uri}: {this.props.track.name}</div>
	}
}
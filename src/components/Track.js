
import React, { PropTypes } from 'react'

export default class Track extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		return <div className="track">#{this.props.track.uri}: {this.props.track.name}</div>
	}
}
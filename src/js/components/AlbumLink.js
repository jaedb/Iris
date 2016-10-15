
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'
import { Link } from 'react-router'

export default class AlbumLink extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		var link = '/album/' + this.props.album.uri;
		return (
			<Link to={link} className="album-link">{ this.props.album.name }</Link>
		);
	}
}
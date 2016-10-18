
import React, { PropTypes } from 'react'
import { Link } from 'react-router'
import Thumbnail from './Thumbnail'

export default class AlbumGridItem extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		var link = '/album/' + this.props.item.uri;
		return (
			<Link to={link} className="grid-item album-grid-item">
				<Thumbnail size="medium" images={this.props.item.images} />
			</Link>
		);
	}
}


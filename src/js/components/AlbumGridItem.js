
import React, { PropTypes } from 'react'
import { Link } from 'react-router'
import Thumbnail from './Thumbnail'

export default class AlbumGridItem extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		var item = this.props.item;
		if( typeof(item.album) !== 'undefined' ){
			item.album.added_at = item.added_at;
			item = item.album;
		}
		var link = '/album/' + item.uri;
		return (
			<Link to={link} className="grid-item album-grid-item">
				{ item.images ? <Thumbnail size="medium" images={item.images} /> : item.name }
			</Link>
		);
	}
}


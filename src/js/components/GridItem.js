
import React, { PropTypes } from 'react'
import { Link, browserHistory } from 'react-router'

import Thumbnail from './Thumbnail'
import ArtistList from './ArtistList'

export default class GridItem extends React.Component{

	constructor(props) {
		super(props);
	}

	handleClick(e){
		if( e.target.tagName.toLowerCase() !== 'a' ){
			browserHistory.push( this.props.link );
		}
	}

	render(){
		var item = this.props.item;
		if( typeof(item.album) !== 'undefined' ){
			item.album.added_at = item.added_at;
			item = item.album;
		}
		return (
			<div className="grid-item" onClick={ (e) => this.handleClick(e) }>
				{ item.images ? <Thumbnail size="medium" images={item.images} /> : item.name }
				<div className="name">{ item.name }</div>
				<div className="secondary">
					{ item.artists ? <ArtistList artists={ item.artists } /> : null }
				</div>
			</div>
		);
	}
}


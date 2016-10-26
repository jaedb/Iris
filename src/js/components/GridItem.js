
import React, { PropTypes } from 'react'
import { Link, browserHistory } from 'react-router'

import Thumbnail from './Thumbnail'
import ArtistSentence from './ArtistSentence'

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
				{ item.images ? <Thumbnail size="medium" images={item.images} /> : null }
				{ item.icons ? <Thumbnail size="medium" images={item.icons} /> : null }
				<div className="name">{ item.name }</div>
				<div className="secondary">
					{ item.artists ? <ArtistSentence artists={ item.artists } /> : null }
					{ item.type == 'playlist' ? item.tracks.total+' tracks' : null }
				</div>
			</div>
		);
	}
}


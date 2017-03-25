
import React, { PropTypes } from 'react'
import { Router, Link, hashHistory } from 'react-router'
import FontAwesome from 'react-fontawesome'

import Thumbnail from './Thumbnail'
import ArtistSentence from './ArtistSentence'

export default class GridItem extends React.Component{

	constructor(props) {
		super(props);
	}

	handleClick(e){
		if( e.target.tagName.toLowerCase() !== 'a' ){
			hashHistory.push(this.props.link)
		}
	}

	renderThumbnail(){
		if( this.props.item.images ) return <Thumbnail size="medium" images={this.props.item.images} />
		if( this.props.item.icons ) return <Thumbnail size="medium" images={this.props.item.icons} />
		return <Thumbnail size="medium" images={[]} />
	}

	renderSecondary(item){
		var output = ''

		switch (item.type){

			case 'playlist':
				return (
					<span>
						{ item.tracks_total ? item.tracks_total+' tracks' : null }
						{ item.can_edit ? <FontAwesome name="edit" /> : null }
					</span>
				)
				break

			case 'artist':
				return (
					<span>
						{ item.followers ? item.followers.total.toLocaleString()+' followers' : item.albums_uris.length+' albums' }
					</span>
				)
				break

			default:
				return (
					<span>
						{ item.artists ? <ArtistSentence artists={ item.artists } /> : null }
						{ item.followers ? item.followers.total.toLocaleString()+' followers' : null }
					</span>
				)
		}

		return output
	}

	render(){
		if( !this.props.item ) return null

		var item = this.props.item;
		if( typeof(item.album) !== 'undefined' ){
			item.album.added_at = item.added_at;
			item = item.album;
		}
		return (
			<div className="grid-item" onClick={ (e) => this.handleClick(e) }>
				{ this.renderThumbnail() }
				<div className="name">{ item.name }</div>
				<div className="secondary">
					{ this.renderSecondary(item) }
				</div>
			</div>
		);
	}
}


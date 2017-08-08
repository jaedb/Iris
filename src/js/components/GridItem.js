
import React, { PropTypes } from 'react'
import ReactDOM from 'react-dom'
import { Router, Link, hashHistory } from 'react-router'
import FontAwesome from 'react-fontawesome'
import LazyLoad from 'react-lazyload'

import Thumbnail from './Thumbnail'
import ArtistSentence from './ArtistSentence'

export default class GridItem extends React.Component{

	constructor(props) {
		super(props)
	}

	handleClick(e){
		if (this.props.onClick && e.target.tagName.toLowerCase() !== 'a'){
			this.props.onClick(e)
		}
	}

	handleContextMenu(e){
		if (this.props.onContextMenu){
			this.props.onContextMenu(e)
		}
	}

	renderSecondary(item){
		var output = ''

		switch (item.type){

			case 'playlist':
				return (
					<span>
						{item.tracks_total ? item.tracks_total+' tracks' : null}
						{item.can_edit ? <FontAwesome name="edit" /> : null}
					</span>
				)
				break

			case 'artist':
				return (
					<span>
						{item.followers ? item.followers.total.toLocaleString()+' followers' : item.albums_uris.length+' albums'}
					</span>
				)
				break

			case 'album':
				return (
					<span>
						{item.artists ? <ArtistSentence artists={item.artists} /> : null}
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
		if (!this.props.item) return null

		var item = this.props.item;
		if( typeof(item.album) !== 'undefined' ){
			item.album.added_at = item.added_at;
			item = item.album;
		}
		var images = []		
		if (this.props.item.images) {
			images = this.props.item.images
		} else if (this.props.item.icons){
			images = this.props.item.icons
		}

		return (
			<div className="grid-item" onClick={e => this.handleClick(e)} onContextMenu={e => this.handleContextMenu(e)}>
				<LazyLoad height={10} placeholder={<Thumbnail size="medium" />}>
					<Thumbnail size="medium" images={images} />
				</LazyLoad>
				<div className="name">{item.name ? item.name : item.uri}</div>
				<div className="secondary">
					{ this.renderSecondary(item) }
				</div>
			</div>
		);
	}
}



import React, { PropTypes } from 'react'
import ReactDOM from 'react-dom'
import { Router, Link, hashHistory } from 'react-router'
import FontAwesome from 'react-fontawesome'

import * as helpers from '../helpers'
import Thumbnail from './Thumbnail'
import ArtistSentence from './ArtistSentence'

export default class GridItem extends React.Component{

	constructor(props){
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

	shouldComponentUpdate(nextProps, nextState){
		return nextProps.item != this.props.item
	}

	renderSecondary(item){
		var output = ''

		switch (item.type){

			case 'playlist':
				return (
					<span>
						{item.tracks_total ? item.tracks_total : 0} tracks
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

		return output;
	}

	render(){
		if (!this.props.item) return null

		var item = this.props.item;
		if (typeof(item.album) !== 'undefined'){
			item.album.added_at = item.added_at;
			item = item.album;
		}
		var images = null
		if (this.props.item.images){
			images = this.props.item.images
		} else if (this.props.item.icons){
			images = this.props.item.icons
		}

		return (
			<div className="grid-item" onClick={e => this.handleClick(e)} onContextMenu={e => this.handleContextMenu(e)}>
				<Thumbnail size="medium" images={images} />
				<div className="name">
					{item.name ? item.name : <span className="dark-grey-text">{item.uri}</span>}
				</div>
				<div className="secondary">					
					{this.props.show_source_icon ? <FontAwesome name={helpers.sourceIcon(item.uri)} className="source" /> : null}
					{this.renderSecondary(item)}
				</div>
			</div>
		);
	}
}


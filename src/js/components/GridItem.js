
import React, { PropTypes } from 'react'
import ReactDOM from 'react-dom'
import { Router, Link, hashHistory } from 'react-router'

import * as helpers from '../helpers'
import Icon from './Icon'
import Thumbnail from './Thumbnail'
import ArtistSentence from './ArtistSentence'

export default class GridItem extends React.Component{

	constructor(props){
		super(props)
	}

	componentDidMount(){

		// A mount callback allows us to run checks on render
		// We use this for loading artwork, but only when it's displayed
		if (this.props.onMount){
			this.props.onMount();
		}
	}

	handleClick(e){
		if (this.props.onClick && e.target.tagName.toLowerCase() !== 'a'){
			this.props.onClick(e);
		}
	}

	handleContextMenu(e){
		if (this.props.onContextMenu){
			this.props.onContextMenu(e)
		}
	}

	shouldComponentUpdate(nextProps, nextState){
		return nextProps.item != this.props.item;
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
						{item.followers ? item.followers.total.toLocaleString()+' followers' : null}
						{item.albums_uris ? item.albums_uris.length+' albums' : null}
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
			<div className={"grid-item "+this.props.type+"-grid-item"} onClick={e => this.handleClick(e)} onContextMenu={e => this.handleContextMenu(e)}>
				<Thumbnail size="medium" images={images} />
				<div className="name">
					{item.name ? item.name : <span className="dark-grey-text">{item.uri}</span>}
				</div>
				<div className="secondary">					
					{this.props.show_source_icon ? <Icon name={helpers.sourceIcon(item.uri)} type="fontawesome" className="source" /> : null}
					{this.renderSecondary(item)}
				</div>
			</div>
		);
	}
}


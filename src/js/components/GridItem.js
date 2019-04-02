
import React from 'react';
import ReactDOM from 'react-dom';

import * as helpers from '../helpers';
import Link from './Link';
import Icon from './Icon';
import Thumbnail from './Thumbnail';
import ArtistSentence from './ArtistSentence';

export default class GridItem extends React.Component{

	constructor(props){
		super(props)
	}

	componentDidMount(){
		if (this.props.item){
			var item = this.props.item;
		} else {
			return;
		}

		// If the item that has just been mounted doesn't have images,
		// try fetching them from LastFM
		if (!item.images && this.props.lastfmActions){
			switch (helpers.uriType(item.uri)){

				case 'artist':
					this.props.lastfmActions.getArtist(item.uri, item.name);
					break;

				case 'album':
					if (item.artists && item.artists.length > 0){
						this.props.lastfmActions.getAlbum(item.uri, item.artists[0].name, item.name, (item.mbid ? item.mbid : null));
					}
					break;
			}
		}
	}

	onContextMenu(e){
		if (this.props.onContextMenu){
			this.props.onContextMenu(e)
		}
	}

	shouldComponentUpdate(nextProps, nextState){
		return nextProps.item != this.props.item;
	}

	renderSecondary(item){
		var output = '';
		var link_to = null;

		switch (helpers.uriType(item.uri)){
			case 'playlist':
				if (item.tracks_total){
					return (
						<span className="grid__item__secondary__content">
							{item.tracks_total} tracks
						</span>
					);
				}
				break

			case 'artist':
				return (
					<span className="grid__item__secondary__content">
						{item.followers !== undefined ? item.followers.toLocaleString()+' followers ' : null}
						{item.albums_uris !== undefined ? item.albums_uris.length+' albums' : null}
					</span>
				)
				break

			case 'album':
				return (
					<span className="grid__item__secondary__content">
						{item.artists !== undefined ? <ArtistSentence nolinks artists={item.artists} /> : null}
					</span>
				)
				break

			default:
				return (
					<span className="grid__item__secondary__content">
						{ item.artists !== undefined ? <ArtistSentence nolinks artists={ item.artists } /> : null }
						{ item.followers !== undefined ? item.followers.toLocaleString()+' followers' : null }
					</span>
				)
		}

		return output;
	}

	render(){
		if (!this.props.item){
			return null;
		}

		var item = this.props.item;
		if (item.album !== undefined){
			item.album.added_at = item.added_at;
			item = item.album;
		}
		var images = null;
		if (this.props.item.images){
			if (Array.isArray(this.props.item.images)){
				images = this.props.item.images[0];
			} else {
				images = this.props.item.images;
			}
		} else if (this.props.item.icons){
			images = this.props.item.icons;
		}

		if (this.props.link){
			var link = this.props.link;
		} else {
			var link = '/'+this.props.type+'/'+encodeURIComponent(item.uri);
		}

		return (
			<Link 
				className={"grid__item grid__item--"+this.props.type} 
				to={link} 
				onClick={e => helpers.scrollTo()} 
				onContextMenu={e => this.onContextMenu(e)}>
					<Thumbnail size="medium" className="grid__item__thumbnail" images={images} />
					<div className="grid__item__name">
						{item.name ? item.name : <span className="opaque-text">{item.uri}</span>}
					</div>
					<div className="grid__item__secondary">					
						{this.props.show_source_icon ? <Icon name={helpers.sourceIcon(item.uri)} type="fontawesome" className="source" /> : null}
						{this.renderSecondary(item)}
					</div>
			</Link>
		);
	}
}


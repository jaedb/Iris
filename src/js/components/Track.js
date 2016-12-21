
import React, { PropTypes } from 'react'
import { Link } from 'react-router'
import FontAwesome from 'react-fontawesome'
import ArtistSentence from './ArtistSentence'
import Dater from './Dater'

import * as helpers from '../helpers'

export default class Track extends React.Component{

	constructor(props) {
		super(props);
	}

	handleMouseDown(e){
		var target = $(e.target);
		if( !target.is('a') && target.closest('a').length <= 0 ){
			this.props.handleMouseDown(e);
		}
	}

	handleTouchEnd(e){
		var target = $(e.target);
		if( !target.is('a') && target.closest('a').length <= 0 ){
			this.props.handleTouchEnd(e);
		}
	}

	handleContextMenu(e){
		e.preventDefault();
		this.props.handleContextMenu(e);
	}

	render(){
		if( !this.props.track ) return null

		var track = this.props.track;
		var className = 'list-item track';
		if( typeof(track.selected) !== 'undefined' && track.selected ) className += ' selected';
		if( track.playing ) className += ' playing';

		var album = '-'
		if( track.album ){
			if( track.album.uri ){
				album = <Link to={global.baseURL+'album/'+track.album.uri}>{track.album.name}</Link>
			} else {
				album = <span>{track.album.name}</span>
			}
		}

		return (
			<div
				className={className}
				onTouchStart={ e => this.props.handleTouchStart(e) }
				onTouchEnd={ e => this.handleTouchEnd(e) }
				onMouseDown={ e => this.handleMouseDown(e) }
				onMouseUp={ e => this.props.handleMouseUp(e) }
				onDoubleClick={ e => this.props.handleDoubleClick(e) }
				onContextMenu={ e => this.handleContextMenu(e) }>
					{ this.props.track.selected ? <FontAwesome name="check" className="select-state" fixedWidth /> : null }
					{ this.props.track.playing ? <FontAwesome name="play" className="play-state" fixedWidth /> : null }
					<span className="col name">
						{ track.name ? track.name : <span className="grey-text">{track.uri}</span> }
					</span>
					<span className="col artists">
						{ track.artists ? <ArtistSentence artists={track.artists} /> : '-' }
					</span>
					<span className="col album">
						{album}
					</span>
					<span className="col duration">
						{ track.duration_ms ? <Dater type="length" data={track.duration_ms} /> : null }
						{ track.length ? <Dater type="length" data={track.length} /> : null }
					</span>
					{ this.props.show_source_icon ? <FontAwesome className="source" name={helpers.sourceIcon(track.uri)} /> : null }
			</div>
		);
	}
}
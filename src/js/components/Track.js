
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
		if( typeof(track.type) !== 'undefined' ) className += ' '+track.type;
		if( track.playing ) className += ' playing';
		
		var album = '-'
		if( track.album ){
			if( track.album.uri ){
				album = <Link to={global.baseURL+'album/'+track.album.uri}>{track.album.name}</Link>
			} else {
				album = <span>{track.album.name}</span>
			}
		}

		if (track.type == 'history'){

			var track_columns = (
				<span>
					<span className="col name">
						{ track.name ? track.name : <span className="grey-text">{track.uri}</span> }
					</span>
					<span className="col source">
						{helpers.uriSource(track.uri)}
					</span>
					<span className="col played_at">
						{ track.played_at ? <span><Dater type="ago" data={track.played_at} /> ago</span> : null }
					</span>
				</span>
			)

		} else if (this.props.context == 'queue'){

				if (track.added_from && track.added_by){
					var type = (track.added_from ? helpers.uriType(track.added_from) : null)
					if (type == 'discover'){
						var link = <Link to={global.baseURL+'discover/recommendations/'+helpers.getFromUri('seeds',track.added_from)}>discover</Link>
					} else {
						var link = <Link to={global.baseURL+type+'/'+track.added_from}>{type}</Link>
					}
					var added = <span>{track.added_by} <span className="grey-text"> (from {link})</span></span>

				} else if (track.added_by){
					var added = track.added_by

				} else {
					var added = '-'
				}

				var track_columns = (
					<span>
						<span className="col name">
							{ track.name ? track.name : <span className="grey-text">{track.uri}</span> }
						</span>
						{this.props.show_source_icon ? <FontAwesome name={helpers.sourceIcon(track.uri)} className="source" fixedWidth /> : null}
						<span className="col artists">
							{ track.artists ? <ArtistSentence artists={track.artists} /> : '-' }
						</span>
						<span className="col album">
							{album}
						</span>
						<span className="col added">
							{added}
						</span>
						<span className="col duration">
							{ track.duration_ms ? <Dater type="length" data={track.duration_ms} /> : null }
							{ track.length ? <Dater type="length" data={track.length} /> : null }
						</span>
					</span>
				)

		} else {

			var track_columns = (
				<span>
					<span className="col name">
						{ track.name ? track.name : <span className="grey-text">{track.uri}</span> }
					</span>
					{this.props.show_source_icon ? <FontAwesome name={helpers.sourceIcon(track.uri)} className="source" fixedWidth /> : null}
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
				</span>
			)
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
					<span className="state-icon">
						{ this.props.track.selected ? <FontAwesome name="check" className="selected" fixedWidth /> : null }
						{ this.props.track.playing && !this.props.track.selected ? <FontAwesome name="play" className="playing" fixedWidth /> : null }
					</span>
					{ track_columns }
			</div>
		);
	}
}
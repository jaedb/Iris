
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'
import ArtistSentence from './ArtistSentence'
import AlbumLink from './AlbumLink'
import Dater from './Dater'
var helpers = require('../helpers.js')

export default class Track extends React.Component{

	constructor(props) {
		super(props);
	}

	handleClick(e){
		var target = $(e.target);
		if( !target.is('a') && target.closest('a').length <= 0 ){
			this.props.handleClick(e);
		}
	}

	handleDoubleClick(e){
		return this.props.handleDoubleClick(e);
	}

	handleContextMenu(e){
		e.preventDefault();

		// trigger a regular click event
		if( !this.props.track.selected ) this.handleClick(e);

		// notify our tracklist
		this.props.handleContextMenu(e);
	}

	render(){
		var track = this.props.track;

		var className = 'list-item track';
		if( typeof(track.selected) !== 'undefined' && track.selected ) className += ' selected';
		if( track.playing ) className += ' playing';

		return (
			<div
				className={className}
				onDoubleClick={ (e) => this.handleDoubleClick(e) }
				onClick={ (e) => this.handleClick(e) }
				onContextMenu={ (e) => this.handleContextMenu(e) }>
					{ this.props.track.selected ? <FontAwesome name="check" className="select-state" fixedWidth /> : null }
					<span className="col name">
						{track.name}
					</span>
					<span className="col artists">
						{ track.artists ? <ArtistSentence artists={track.artists} /> : null }
					</span>
					<span className="col album">
						{ track.album ? <AlbumLink album={track.album} /> : null }
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
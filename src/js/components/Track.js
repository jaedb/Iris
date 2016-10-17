
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'
import ArtistList from './ArtistList'
import AlbumLink from './AlbumLink'

export default class Track extends React.Component{

	constructor(props) {
		super(props);
	}

	handleClick( e ){
		var target = $(e.target);
		if( !target.is('a') && target.closest('a').length <= 0 ){
			this.props.handleClick(e);
		}
	}

	handleDoubleClick( e ){
		return this.props.handleDoubleClick(e);
	}

	render(){

		var selectedIcon = 'square-o';
		if( typeof(this.props.track.selected) !== 'undefined' && this.props.track.selected ){
			selectedIcon = 'check-square-o';
		}

		return (
			<div
				className="track"
				onDoubleClick={ (e) => this.handleDoubleClick(e) }
				onClick={ (e) => this.handleClick(e) }>
					<FontAwesome name={selectedIcon} fixedWidth />
					<span className="name">
						{this.props.track.name}
					</span>
					<span className="artists">
						<ArtistList artists={this.props.track.artists} />
					</span>
					<span className="album">
						{ this.props.track.album ? <AlbumLink album={this.props.track.album} /> : null }
					</span>
					<span className="duration">
						{ this.props.track.duration }
					</span>
			</div>
		);
	}
}
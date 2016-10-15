
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'
import ArtistList from './ArtistList'
import AlbumLink from './AlbumLink'

export default class Track extends React.Component{

	constructor(props) {
		super(props);
	}

	toggleSelected( e ){
		return this.props.toggleSelected();
	}

	playTrack( e ){
		return this.props.playTrack();
	}

	render(){

		var selectedIcon = 'square-o';
		if( typeof(this.props.track.selected) !== 'undefined' && this.props.track.selected ){
			selectedIcon = 'check-square-o';
		}

		return (
			<div
				className="track"
				onDoubleClick={ (e) => this.playTrack(e) }>
					<FontAwesome name={selectedIcon} fixedWidth onClick={ (e) => this.toggleSelected(e) } />
					{this.props.track.name}
					<ArtistList artists={this.props.track.artists} />
					{ this.props.track.album ? <AlbumLink album={this.props.track.album} /> : null }
			</div>
		);
	}
}
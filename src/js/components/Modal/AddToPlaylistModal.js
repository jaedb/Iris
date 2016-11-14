
import React, { PropTypes } from 'react'
import Icon from '../Icon'
import * as helpers from '../../helpers'

class AddToPlaylistModal extends React.Component{

	constructor(props){
		super(props)
	}

	playlistSelected( playlist_uri ){
		this.props.uiActions.addTracksToPlaylist( playlist_uri, this.props.data.track_indexes )
		this.props.uiActions.closeModal()
	}

	render(){
		if( !this.props.playlists ) return <div className="empty">No editable playlists</div>
		var playlists = []
		for( var i = 0; i < this.props.playlists.length; i++ ){
			if( this.props.playlists[i].can_edit ) playlists.push( this.props.playlists[i] )
		}

		return (
			<div className="playlists">
				{
					playlists.map( playlist => {
						return (
							<div className="playlist" key={playlist.uri} onClick={ () => this.playlistSelected(playlist.uri) }>
								{ playlist.name }
							</div>
						)
					})
				}
			</div>
		)
	}
}
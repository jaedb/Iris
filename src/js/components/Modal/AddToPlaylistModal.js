
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'
import Icon from '../Icon'
import * as helpers from '../../helpers'

export default class AddToPlaylistModal extends React.Component{

	constructor(props){
		super(props)
	}

	playlistSelected( playlist_uri ){
		this.props.uiActions.addTracksToPlaylist( playlist_uri, this.props.tracks_uris )
		this.props.uiActions.closeModal()
	}

	render(){
		if( !this.props.playlists ) return <div className="empty">No editable playlists</div>
		var playlists = []
		for( var i = 0; i < this.props.playlists.length; i++ ){
			if( this.props.playlists[i].can_edit ) playlists.push( this.props.playlists[i] )
		}

		return (
			<div>
				<h4>Add to playlist</h4>
				<div className="playlists">
					{
						playlists.map( playlist => {
							return (
								<div className="playlist" key={playlist.uri} onClick={ () => this.playlistSelected(playlist.uri) }>
									<FontAwesome className="source" name={helpers.sourceIcon(playlist.uri)} />
									&nbsp;
									<span className="name">{ playlist.name }</span>
									{ playlist.tracks ? <span className="grey-text">&nbsp;{ playlist.tracks.total } tracks</span> : null }
								</div>
							)
						})
					}
				</div>
			</div>
		)
	}
}

import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

import Icon from '../Icon'
import Thumbnail from '../Thumbnail'
import * as helpers from '../../helpers'

export default class AddToPlaylistModal extends React.Component{

	constructor(props){
		super(props)

		if (!this.props.spotify_library_playlists){
			this.props.spotifyActions.getLibraryPlaylists()
		}
		
		if (!this.props.mopidy_library_playlists){
			this.props.mopidyActions.getLibraryPlaylists()
		}
	}

	playlistSelected( playlist_uri ){
		this.props.coreActions.addTracksToPlaylist( playlist_uri, this.props.tracks_uris )
		this.props.uiActions.closeModal()
	}

	render(){
		if( !this.props.playlists ) return <div className="empty">No editable playlists</div>
		var playlists = []
		for (var uri in this.props.playlists){
			if( this.props.playlists[uri].can_edit ) playlists.push( this.props.playlists[uri] )
		}

		playlists = helpers.sortItems(playlists, 'name')

		var loader = null
		if (this.props.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR && this.props.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR.status == 'running'){
			loader = (
				<div className='lazy-loader body-loader loading'>
					<div className="loader"></div>
				</div>
			)
		}

		return (
			<div>
				<h1>Add to playlist</h1>
				<h2 className="grey-text">Select playlist to add {this.props.tracks_uris.length} track{this.props.tracks_uris.length>1?'s':null} to</h2>
				{playlists.length <= 0 ? <div className="no-results">No playlists available</div> : null}
				<div className="list small playlists">
					{
						playlists.map( playlist => {
							return (
								<div className="list-item" key={playlist.uri} onClick={ () => this.playlistSelected(playlist.uri) }>
									<Thumbnail images={playlist.images} size="small" />
									<h3 className="name">{ playlist.name }</h3>
									<ul className="details">
										<li><FontAwesome className="source" name={helpers.sourceIcon(playlist.uri)} /></li>
										<li>{ playlist.tracks_total ? <span className="grey-text">&nbsp;{ playlist.tracks_total } tracks</span> : null }</li>
									</ul>
								</div>
							)
						})
					}
				</div>
				{loader}
			</div>
		)
	}
}
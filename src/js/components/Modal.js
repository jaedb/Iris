
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Icon from './Icon'
import * as helpers from '../helpers'

import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Modal extends React.Component{

	playlistSelected( playlist_uri ){
		this.props.uiActions.addTracksToPlaylist( playlist_uri, this.props.modal.data.track_indexes )
		this.props.uiActions.closeModal()
	}

	renderEditablePlaylists(){
		if( !this.props.playlists ) return <div className="empty">No editable playlists</div>
		var playlists = []
		for( var i = 0; i < this.props.playlists.length; i++ ){
			switch( helpers.uriSource( this.props.playlists[i].uri ) ){

				case 'spotify':
					if( this.props.playlists[i].can_edit ) playlists.push( this.props.playlists[i] )
					break

				case 'm3u':
					playlists.push( this.props.playlists[i] )
					break

			}
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

	render(){
		if( !this.props.modal ) return null;

		return (
			<div className="modal">
				<div className="close-modal" onClick={ () => this.props.uiActions.closeModal() }>
					<Icon name="close" className="white" />
				</div>
				<div className="content">

					<h1>{ this.props.modal.name }</h1>

					{ this.renderEditablePlaylists() }

				</div>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		modal: state.ui.modal,
		playlists: state.ui.playlists,
		context_menu: state.ui.context_menu,
		mopidy_connected: state.mopidy.connected,
		spotify_authorized: state.spotify.authorized
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Modal)

import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Icon from './Icon'
import * as helpers from '../helpers'

import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Modal extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			playlist_name: '',
			playlist_scheme: 'spotify',
			playlist_private: true
		}
	}

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

	createPlaylist(e){		
		e.preventDefault();
		this.props.uiActions.createPlaylist( this.state.playlist_scheme, this.state.playlist_name, this.state.playlist_public );
		return false;
	}

	renderCreatePlaylist(){
		return (
			<div>
				<h1>Create playlist</h1>
				<form onSubmit={(e) => this.createPlaylist(e)}>
					<div className="field">
						<div className="name">Name</div>
						<div className="input">
							<input 
								type="text"
								onChange={ e => this.setState({ playlist_name: e.target.value })} 
								value={ this.state.playlist_name } />
						</div>
					</div>
					<div className="field radio white">
						<div className="name">Provider</div>
						<div className="input">
							<label>
								<input 
									type="radio"
									name="playlist_scheme"
									value="spotify"
									checked={ this.state.playlist_scheme == 'spotify' }
									onChange={ e => this.setState({ playlist_scheme: e.target.value })} />
								<span className="label">Spotify</span>
							</label>
							<label>
								<input 
									type="radio"
									name="playlist_scheme"
									value="m3u"
									checked={ this.state.playlist_scheme == 'm3u' }
									onChange={ e => this.setState({ playlist_scheme: e.target.value })} />
								<span className="label">Local</span>
							</label>
						</div>
					</div>
					<div className="field checkbox white">
						<div className="name">Options</div>
						<div className="input">
							<label>
								<input 
									type="checkbox"
									name="playlist_private"
									value={ this.state.playlist_private }
									onChange={ e => this.setState({ playlist_private: !this.state.playlist_private })} />
								<span className="label">Private</span>
							</label>
						</div>
					</div>
					<button type="submit" className="primary">Save</button>
				</form>
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

					{ this.props.modal.name == 'add_to_playlist' ? this.renderEditablePlaylists() : null }
					{ this.props.modal.name == 'create_playlist' ? this.renderCreatePlaylist() : null }

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

import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'
import ReactGA from 'react-ga'

import Modal from './Modal';
import Icon from '../../components/Icon';
import Thumbnail from '../../components/Thumbnail';
import * as coreActions from '../../services/core/actions'
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'
import * as helpers from '../../helpers';

class AddToPlaylist extends React.Component{

	constructor(props){
		super(props)

		if (!this.props.spotify_library_playlists){
			this.props.spotifyActions.getLibraryPlaylists()
		}
		
		if (!this.props.mopidy_library_playlists && this.props.mopidy_connected){
			this.props.mopidyActions.getLibraryPlaylists()
		}
	}

	playlistSelected(playlist_uri){
		this.props.coreActions.addTracksToPlaylist(playlist_uri, this.props.uris);
		window.history.back();
	}

	render(){
		if (!this.props.playlists ) return <div className="empty">No editable playlists</div>
		var playlists = []
		for (var uri in this.props.playlists){
			if (this.props.playlists[uri].can_edit ) playlists.push(this.props.playlists[uri] )
		}

		playlists = helpers.sortItems(playlists, 'name')

		var loader = null
		if (this.props.spotify_library_playlists_status == 'running'){
			loader = (
				<div className='lazy-loader body-loader loading'>
					<div className="loader"></div>
				</div>
			)
		}

		return (
			<Modal className="modal--add-to-playlist">
				<h1>Add to playlist</h1>
				<h2 className="mid_grey-text">Select playlist to add {this.props.uris.length} track{this.props.uris.length>1?'s':null} to</h2>
				{playlists.length <= 0 ? <div className="no-results">No playlists available</div> : null}
				<div className="list small playlists">
					{
						playlists.map(playlist => {
							return (
								<div className="list__item" key={playlist.uri} onClick={ () => this.playlistSelected(playlist.uri) }>
									<Thumbnail images={playlist.images} size="small" />
									<h3 className="name">{ playlist.name }</h3>
									<ul className="details">
										<li><Icon type="fontawesome" className="source" name={helpers.sourceIcon(playlist.uri)} /></li>
										<li>{ playlist.tracks_total ? <span className="mid_grey-text">&nbsp;{ playlist.tracks_total } tracks</span> : null }</li>
									</ul>
								</div>
							)
						})
					}
				</div>
				{loader}
			</Modal>
		)
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		uris: (ownProps.params.uris ? decodeURIComponent(ownProps.params.uris).split(',') : []),
		mopidy_connected: state.mopidy.connected,
		mopidy_uri_schemes: state.mopidy.uri_schemes,
		mopidy_library_playlists: state.mopidy.library_playlists,
		mopidy_library_playlists_status: (state.ui.processes.MOPIDY_LIBRARY_PLAYLISTS_PROCESSOR !== undefined ? state.ui.processes.MOPIDY_LIBRARY_PLAYLISTS_PROCESSOR.status : null),
		spotify_library_playlists: state.spotify.library_playlists,
		spotify_library_playlists_status: (state.ui.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR !== undefined ? state.ui.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR.status : null),
		load_queue: state.ui.load_queue,
		me_id: (state.spotify.me ? state.spotify.me.id : false),
		playlists: state.core.playlists
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(AddToPlaylist)
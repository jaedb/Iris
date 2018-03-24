
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import * as helpers from '../../helpers'
import Icon from '../Icon'
import AddToPlaylistModal from './AddToPlaylistModal'
import AddToQueueModal from './AddToQueueModal'
import CreatePlaylistModal from './CreatePlaylistModal'
import EditPlaylistModal from './EditPlaylistModal'
import EditRadioModal from './EditRadioModal'
import EditSnapcastClientModal from './EditSnapcastClientModal'
import ImageZoomModal from './ImageZoomModal'
import KioskModeModal from './KioskModeModal'
import SearchURISchemesModal from './SearchURISchemesModal'
import InitialSetupModal from './InitialSetupModal'
import AuthorizationModal_Send from './AuthorizationModal_Send'
import AuthorizationModal_Receive from './AuthorizationModal_Receive'

import * as coreActions from '../../services/core/actions'
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'
import * as pusherActions from '../../services/pusher/actions'

class Modal extends React.Component{

	constructor(props){
		super(props)
	}

	componentWillReceiveProps(nextProps){
		if (nextProps.modal){
			$('body').addClass('modal-open')
		} else {
			$('body').removeClass('modal-open')
		}
	}

	toggleFullscreen(){
		let app = document.documentElement
		if (app.requestFullscreen){
			app.requestFullscreen()
		} else if (app.webkitRequestFullscreen){
			app.webkitRequestFullscreen()
		} else if (app.mozRequestFullScreen){
			app.mozRequestFullScreen()
		} else if (app.msRequestFullscreen){
			app.msRequestFullscreen()
		}
	}

	render(){
		if (!this.props.modal){
			return null;
		}

		var can_fullscreen = false;
		if (this.props.modal.name == 'kiosk_mode'){
			can_fullscreen = true;
		}

		return (
			<div className={this.props.modal.name+" modal"}>

				<div className="controls">
					<div className="control close" onClick={e => this.props.uiActions.closeModal()}>
						<Icon name="close" className="white" />
					</div>
				</div>

				<div className="content">

					{ this.props.modal.name == 'add_to_playlist' ? <AddToPlaylistModal 
						uiActions={this.props.uiActions} 
						coreActions={this.props.coreActions} 
						spotifyActions={this.props.spotifyActions} 
						mopidyActions={this.props.mopidyActions} 
						playlists={this.props.playlists} 
						tracks_uris={this.props.modal.data.tracks_uris} 
						spotify_library_playlists={this.props.spotify_library_playlists} 
						mopidy_library_playlists={this.props.mopidy_library_playlists} 
						processes={this.props.processes} /> : null }

					{ this.props.modal.name == 'add_to_queue' ? <AddToQueueModal
						uiActions={this.props.uiActions} 
						mopidyActions={this.props.mopidyActions} /> : null }

					{ this.props.modal.name == 'create_playlist' ? <CreatePlaylistModal
						uiActions={this.props.uiActions}
						coreActions={this.props.coreActions} /> : null }

					{ this.props.modal.name == 'edit_playlist' ? <EditPlaylistModal
						uiActions={this.props.uiActions}
						coreActions={this.props.coreActions}
						data={this.props.modal.data} /> : null }

					{ this.props.modal.name == 'send_authorization' ? <AuthorizationModal_Send
						uiActions={this.props.uiActions}
						pusherActions={this.props.pusherActions}
						data={this.props.modal.data} /> : null }

					{ this.props.modal.name == 'receive_authorization' ? <AuthorizationModal_Receive 
						uiActions={this.props.uiActions} 
						spotifyActions={this.props.spotifyActions} 
						data={this.props.modal.data} /> : null }

					{ this.props.modal.name == 'edit_radio' ? <EditRadioModal 
						uiActions={this.props.uiActions} 
						pusherActions={this.props.pusherActions} 
						spotifyActions={this.props.spotifyActions} 
						data={this.props.modal.data} 
						radio={this.props.radio} 
						artists={this.props.artists} 
						tracks={this.props.tracks} /> : null }

					{ this.props.modal.name == 'edit_snapcast_client' ? <EditSnapcastClientModal 
						uiActions={this.props.uiActions} 
						pusherActions={this.props.pusherActions} 
						data={this.props.modal.data} 
						clients={this.props.snapcast_clients} 
						groups={this.props.snapcast_groups} /> : null }

					{ this.props.modal.name == 'image_zoom' ? <ImageZoomModal 
						uiActions={this.props.uiActions} 
						data={this.props.modal.data} /> : null }

					{ this.props.modal.name == 'kiosk_mode' ? <KioskModeModal 
						uiActions={this.props.uiActions} 
						data={this.props.modal.data} 
						current_track={this.props.current_track} /> : null }

					{ this.props.modal.name == 'search_uri_schemes' ? <SearchURISchemesModal 
						uiActions={this.props.uiActions} 
						coreActions={this.props.coreActions} 
						search_uri_schemes={this.props.search_uri_schemes} 
						available_uri_schemes={this.props.uri_schemes} 
						data={this.props.modal.data} /> : null }

					{ this.props.modal.name == 'initial_setup' ? <InitialSetupModal
						uiActions={this.props.uiActions}
						pusherActions={this.props.pusherActions} /> : null }

				</div>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		current_track: (state.core.current_track && state.core.tracks[state.core.current_track.uri] !== undefined ? state.core.tracks[state.core.current_track.uri] : null),
		uri_schemes: (state.mopidy.uri_schemes ? state.mopidy.uri_schemes : []),
		search_uri_schemes: (state.ui.search_uri_schemes ? state.ui.search_uri_schemes : []),
		volume: state.mopidy.volume,
		mute: state.mopidy.mute,
		modal: state.ui.modal,
		radio: state.core.radio,
		tracks: state.core.tracks,
		artists: state.core.artists,
		playlists: state.core.playlists,
		context_menu: state.ui.context_menu,
		processes: state.ui.processes,
		load_queue: state.ui.load_queue,
		mopidy_connected: state.mopidy.connected,
		spotify_authorized: state.spotify.authorization,
		spotify_library_playlists: state.spotify.library_playlists,
		mopidy_library_playlists: state.mopidy.library_playlists,
		snapcast_groups: state.pusher.snapcast_groups,
		snapcast_clients: state.pusher.snapcast_clients
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Modal)
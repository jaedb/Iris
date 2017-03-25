
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import * as helpers from '../../helpers'
import Icon from '../Icon'
import AddToPlaylistModal from './AddToPlaylistModal'
import AddToQueueModal from './AddToQueueModal'
import CreatePlaylistModal from './CreatePlaylistModal'
import EditPlaylistModal from './EditPlaylistModal'
import SendAuthorizationModal from './SendAuthorizationModal'
import EditRadioModal from './EditRadioModal'
import ImageZoomModal from './ImageZoomModal'
import KioskModeModal from './KioskModeModal'
import SearchSettingsModal from './SearchSettingsModal'

import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'
import * as pusherActions from '../../services/pusher/actions'

class Modal extends React.Component{

	constructor(props){
		super(props)
	}

	componentWillReceiveProps( nextProps ){
		if (nextProps.modal){
			$('body').addClass('modal-open')
		} else {
			$('body').removeClass('modal-open')
		}
	}

	render(){
		if( !this.props.modal ) return null;

		return (
			<div className={this.props.modal.name+" modal"}>
				<div className="close-modal" onClick={ () => this.props.uiActions.closeModal() }>
					<Icon name="close" className="white" />
				</div>
				<div className="content">

					{ this.props.modal.name == 'add_to_playlist' ? <AddToPlaylistModal uiActions={this.props.uiActions} playlists={this.props.playlists} tracks_uris={this.props.modal.data.tracks_uris} /> : null }
					{ this.props.modal.name == 'add_to_queue' ? <AddToQueueModal uiActions={this.props.uiActions} mopidyActions={this.props.mopidyActions} /> : null }
					{ this.props.modal.name == 'create_playlist' ? <CreatePlaylistModal uiActions={this.props.uiActions} /> : null }
					{ this.props.modal.name == 'edit_playlist' ? <EditPlaylistModal uiActions={this.props.uiActions} data={this.props.modal.data} /> : null }
					{ this.props.modal.name == 'send_authorization' ? <SendAuthorizationModal uiActions={this.props.uiActions} pusherActions={this.props.pusherActions} data={this.props.modal.data} /> : null }
					{ this.props.modal.name == 'edit_radio' ? <EditRadioModal uiActions={this.props.uiActions} pusherActions={this.props.pusherActions} spotifyActions={this.props.spotifyActions} data={this.props.modal.data} radio={this.props.radio} artists={this.props.artists} tracks={this.props.tracks} /> : null }
					{ this.props.modal.name == 'image_zoom' ? <ImageZoomModal uiActions={this.props.uiActions} data={this.props.modal.data} /> : null }
					{ this.props.modal.name == 'kiosk_mode' ? <KioskModeModal uiActions={this.props.uiActions} data={this.props.modal.data} /> : null }
					{ this.props.modal.name == 'search_settings' ? <SearchSettingsModal uiActions={this.props.uiActions} search_settings={this.props.search_settings} uri_schemes={this.props.uri_schemes} data={this.props.modal.data} /> : null }

				</div>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		uri_schemes: (state.mopidy.uri_schemes ? state.mopidy.uri_schemes : null),
		search_settings: (state.ui.search_settings ? state.ui.search_settings : null),
		modal: state.ui.modal,
		radio: state.ui.radio,
		tracks: state.ui.tracks,
		artists: state.ui.artists,
		playlists: state.ui.playlists,
		context_menu: state.ui.context_menu,
		mopidy_connected: state.mopidy.connected,
		spotify_authorized: state.spotify.authorized
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Modal)

import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import * as helpers from '../../helpers'
import Icon from '../Icon'
import AddToPlaylistModal from './AddToPlaylistModal'
import CreatePlaylistModal from './CreatePlaylistModal'
import EditPlaylistModal from './EditPlaylistModal'

import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class Modal extends React.Component{

	constructor(props){
		super(props)
	}

	render(){
		if( !this.props.modal ) return null;

		return (
			<div className="modal">
				<div className="close-modal" onClick={ () => this.props.uiActions.closeModal() }>
					<Icon name="close" className="white" />
				</div>
				<div className="content">

					{ this.props.modal.name == 'add_to_playlist' ? <AddToPlaylistModal /> : null }
					{ this.props.modal.name == 'create_playlist' ? <CreatePlaylistModal /> : null }
					{ this.props.modal.name == 'edit_playlist' ? <EditPlaylistModal uiActions={this.props.uiActions} data={this.props.modal.data} /> : null }

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
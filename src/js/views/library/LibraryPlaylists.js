
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'
import FontAwesome from 'react-fontawesome'

import PlaylistGrid from '../../components/PlaylistGrid'
import Header from '../../components/Header'

import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryPlaylists extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		if( !this.props.playlists ) return null
		var columns = [
			{ name: 'name', width: '30'},
			{ name: 'tracks.total', width: '15'},
			{ name: 'can_edit', width: '15'},
			{ name: 'uri', width: '40'}
		]

		var actions = (
			<button onClick={ () => this.props.uiActions.openModal('create_playlist', {} ) }>
				<FontAwesome name="plus" />&nbsp;
				New
			</button>
		)

		return (
			<div className="view library-playlists-view">

				<Header icon="playlist" title="My playlists" actions={actions} />

				<section className="grid-wrapper">
					<PlaylistGrid playlists={this.props.playlists} />
				</section>

			</div>
		)
	}
}


/**
 * Export our component
 *
 * We also integrate our global store, using connect()
 **/

const mapStateToProps = (state, ownProps) => {
	return {
		playlists: state.ui.playlists
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryPlaylists)
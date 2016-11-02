
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import List from '../../components/List'
import Header from '../../components/Header'

import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryPlaylists extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		if( this.props.mopidy.connected ){
			this.props.mopidyActions.getPlaylists();
		}
		if( this.props.spotify.authorized ){
		this.props.spotifyActions.getLibraryPlaylists();
		}
	}

	componentWillReceiveProps( nextProps ){
		if( !this.props.mopidy.connected && nextProps.mopidy.connected ){
			this.props.mopidyActions.getPlaylists();
		}
		if( !this.props.spotify.authorized && nextProps.spotify.authorized ){
			this.props.spotifyActions.getLibraryPlaylists();
		}
	}

	compiledPlaylistSources(){
		var playlists = [];

		if( this.props.mopidy.playlists ){
			playlists = [...playlists, ...this.props.mopidy.playlists]
		}

		if( this.props.spotify.library_playlists ){
			playlists = [...playlists, ...this.props.spotify.library_playlists.items]
		}

		return playlists;
	}

	renderPlaylists(){
		var playlists = this.compiledPlaylistSources()
		if( !playlists ) return null

		var columns = [
			{ name: 'name', width: '50'},
			{ name: 'uri', width: '25'}
		]

		return <List columns={columns} rows={playlists} link_prefix="/playlist/" />
	}

	render(){
		return (
			<div className="view library-playlists-view">
				<Header
					icon="playlist"
					title="My playlists"
					/>
				{ this.renderPlaylists() }
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
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryPlaylists)
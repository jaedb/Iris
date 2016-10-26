
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import PlaylistListItem from '../../components/PlaylistListItem'
import Header from '../../components/Header'

import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryPlaylists extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		if( this.props.mopidy.connected ) this.loadPlaylists();
	}

	componentWillReceiveProps( nextProps ){
		if( !this.props.mopidy.connected && nextProps.mopidy.connected ) this.loadPlaylists();
	}

	loadPlaylists(){
		this.props.spotifyActions.getLibraryPlaylists();
		this.props.mopidyActions.getPlaylists();
	}

	compiledPlaylistSources(){
		var playlists = [];

		if( this.props.mopidy.playlists ){
			Object.assign(playlists, this.props.mopidy.playlists)
		}

		if( this.props.spotify.library_playlists ){
			Object.assign(playlists, this.props.spotify.library_playlists.items)
		}

		return playlists;
	}

	renderPlaylists(){
		if( !this.compiledPlaylistSources() ) return null;

		return (
			<ul>
				<li className="list-item header playlist">
					<span className="col name">Name</span>
					<span className="col owner">Owner</span>
					<span className="col source">Source</span>
				</li>
				{
					this.compiledPlaylistSources().map( (playlist, index) => {
						return (
							<PlaylistListItem key={index} item={playlist} />
						);
					})
				}
			</ul>
		);
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

import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import Header from '../components/Header'
import TrackList from '../components/TrackList'
import ArtistGrid from '../components/ArtistGrid'
import AlbumGrid from '../components/AlbumGrid'
import PlaylistGrid from '../components/PlaylistGrid'

import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Search extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.performSearch()
	}

	componentWillReceiveProps(newProps){
		if( this.props.params.query != newProps.params.query ){
			this.performSearch( newProps )
		}
	}

	performSearch( props = this.props ){
		this.props.spotifyActions.getSearchResults( props.params.query )
	}

	compiledTracks(){
		var tracks = [];
		if( this.props.mopidy.search_results ) tracks = [...tracks, ...this.props.mopidy.search_results.tracks]
		if( this.props.spotify.search_results ) tracks = [...tracks, ...this.props.spotify.search_results.tracks.items]
		return tracks;
	}

	compiledArtists(){
		var artists = [];
		if( this.props.mopidy.search_results ) artists = [...artists, ...this.props.mopidy.search_results.artists]
		if( this.props.spotify.search_results ) artists = [...artists, ...this.props.spotify.search_results.artists.items]
		return artists.splice(0,9)
	}

	compiledAlbums(){
		var albums = [];
		if( this.props.mopidy.search_results ) tracks = [...albums, ...this.props.mopidy.search_results.albums]
		if( this.props.spotify.search_results ) albums = [...albums, ...this.props.spotify.search_results.albums.items]
		return albums.splice(0,9)
	}

	compiledPlaylists(){
		var playlists = [];
		if( this.props.mopidy.search_results ) playlists = [...playlists, ...this.props.mopidy.search_results.playlists]
		if( this.props.spotify.search_results ) playlists = [...playlists, ...this.props.spotify.search_results.playlists.items]
		return playlists.splice(0,9)
	}

	render(){
		return (
			<div className="view search-view">
				<Header
					icon="search"
					title="Search results"
					/>

				<div className="cf">
					<div className="col w33">
						<h4>Artists</h4>
						<ArtistGrid className="mini" artists={ this.compiledArtists() } />
					</div>
					<div className="col w33">
						<h4 >Albums</h4>
						<AlbumGrid className="mini" albums={ this.compiledAlbums() } />
					</div>
					<div className="col w33">
						<h4>Playlists</h4>
						<PlaylistGrid className="mini" playlists={ this.compiledPlaylists() } />
					</div>
				</div>

				<TrackList tracks={ this.compiledTracks() } />
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Search)
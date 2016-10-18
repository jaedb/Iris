
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import TrackList from '../components/TrackList'
import AlbumGrid from '../components/AlbumGrid'

import * as spotifyActions from '../services/spotify/actions'

class Artist extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.props.spotifyActions.loadArtist( this.props.params.uri );
	}

	// when props changed
	componentWillReceiveProps( nextProps ){
		if( nextProps.params.uri != this.props.params.uri ){
			this.props.spotifyActions.loadArtist( nextProps.params.uri );
		}
	}

	render(){
		if( this.props.spotify.artist ){
			return (
				<div>
					<h3>{ this.props.spotify.artist.name }</h3>
					<p>{ this.props.spotify.artist.followers.total.toLocaleString() } followers</p>
					{ this.props.spotify.artist_top_tracks ? <TrackList tracks={ this.props.spotify.artist_top_tracks.tracks } /> : null }
					{ this.props.spotify.artist_albums ? <AlbumGrid items={ this.props.spotify.artist_albums } /> : null }
				</div>
			);
		}
		return null;
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
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Artist)

import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Header from '../components/Header'
import TrackList from '../components/TrackList'
import AlbumGrid from '../components/AlbumGrid'
import Thumbnail from '../components/Thumbnail'
import Parallax from '../components/Parallax'
import RelatedArtistList from '../components/RelatedArtistList'

import * as spotifyActions from '../services/spotify/actions'

class Artist extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.props.spotifyActions.getArtist( this.props.params.uri );
	}

	// when props changed
	componentWillReceiveProps( nextProps ){
		if( nextProps.params.uri != this.props.params.uri ){
			this.props.spotifyActions.getArtist( nextProps.params.uri );
		}
	}

	render(){
		if( this.props.spotify.artist ){
			return (
				<div>
					<Header icon="mic" title={ this.props.spotify.artist.name } />
					{ this.props.spotify.artist.images ? <Parallax images={ this.props.spotify.artist.images } /> : null }
					{ this.props.spotify.artist.images ? <Thumbnail size="huge" images={ this.props.spotify.artist.images } /> : null }
					<p>{ this.props.spotify.artist.followers.total.toLocaleString() } followers</p>
					{ this.props.spotify.artist.related_artists ? <RelatedArtistList artists={ this.props.spotify.artist.related_artists } /> : null }
					{ this.props.spotify.artist.tracks ? <TrackList tracks={ this.props.spotify.artist.tracks } /> : null }
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
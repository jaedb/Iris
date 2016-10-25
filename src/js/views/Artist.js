
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Header from '../components/Header'
import TrackList from '../components/TrackList'
import AlbumGrid from '../components/AlbumGrid'
import Thumbnail from '../components/Thumbnail'
import Parallax from '../components/Parallax'
import ArtistList from '../components/ArtistList'

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
					<Parallax images={ this.props.spotify.artist.images } />

					<div className="intro">
						<Thumbnail size="huge" images={ this.props.spotify.artist.images } />
						<h1>{ this.props.spotify.artist.name }</h1>
						<p>{ this.props.spotify.artist.followers.total.toLocaleString() } followers</p>
					</div>

					<div className="col w70">
						<h3 className="left-padding">Top tracks</h3>
						{ this.props.spotify.artist.tracks ? <TrackList tracks={ this.props.spotify.artist.tracks } /> : null }
					</div>

					<div className="col w5"></div>

					<div className="col w25 cf">
						<h3>Related artists</h3>
						{ this.props.spotify.artist.related_artists ? <ArtistList artists={ this.props.spotify.artist.related_artists.slice(0,6) } /> : null }
					</div>

					<h3 className="left-padding">Albums</h3>
					{ this.props.spotify.artist_albums ? <AlbumGrid className="no-top-padding" albums={ this.props.spotify.artist_albums.items } /> : null }
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
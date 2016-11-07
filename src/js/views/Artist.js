
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'
let helpers = require('../helpers.js')

import LazyLoadListener from '../components/LazyLoadListener'
import Header from '../components/Header'
import TrackList from '../components/TrackList'
import AlbumGrid from '../components/AlbumGrid'
import Thumbnail from '../components/Thumbnail'
import Parallax from '../components/Parallax'
import ArtistList from '../components/ArtistList'

import * as mopidyActions from '../services/mopidy/actions'
import * as lastfmActions from '../services/lastfm/actions'
import * as spotifyActions from '../services/spotify/actions'

class Artist extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.loadArtist();
	}

	componentWillReceiveProps( nextProps ){
		if( nextProps.params.uri != this.props.params.uri ){
			this.loadArtist( nextProps )
		}else if( !this.props.mopidy.connected && nextProps.mopidy.connected ){
			if( helpers.uriSource( this.props.params.uri ) == 'local' ){
				this.loadArtist( nextProps )
			}
		}
	}

	loadArtist( props = this.props ){
		var source = helpers.uriSource( props.params.uri );
		if( source == 'spotify' ){
			this.props.spotifyActions.getArtist( props.params.uri );
		}else if( source == 'local' && props.mopidy.connected ){
			this.props.mopidyActions.getArtist( props.params.uri );
		}
	}

	loadMore(){
		if( !this.props.spotify.artist_albums || !this.props.spotify.artist_albums.next ) return
		this.props.spotifyActions.getURL( this.props.spotify.artist_albums.next, 'SPOTIFY_ARTIST_ALBUMS_LOADED_MORE' );
	}

	renderSpotifyArtist(){
		if( !this.props.spotify.artist ) return null
		var artist = this.props.spotify.artist
		var albums = this.props.spotify.artist_albums

		return (
			<div className="view artist-view">
				<Parallax images={artist.images} />

				<div className="intro">
					<Thumbnail size="huge" images={ artist.images } />
					<h1>{ artist.name }</h1>
					<ul className="details">
						<li>{ artist.followers.total.toLocaleString() } followers</li>
						{ artist.popularity ? <li>{ artist.popularity }% popularity</li> : null }
						<li><FontAwesome name='spotify' /> Spotify artist</li>
					</ul>
				</div>

				<div className="col w70">
					<h4 className="left-padding">Top tracks</h4>
					{ artist.tracks ? <TrackList tracks={ artist.tracks } /> : null }
				</div>

				<div className="col w5"></div>

				<div className="col w25">
					<h4>Related artists</h4>
					{ artist.related_artists ? <ArtistList artists={ artist.related_artists.slice(0,6) } /> : null }
				</div>

				<div className="cf"></div>

				<h4 className="left-padding">Albums</h4>
				<section className="grid-wrapper no-top-padding">
					{ albums ? <AlbumGrid albums={ albums.items } /> : null }
					<LazyLoadListener loadMore={ () => this.loadMore() }/>
				</section>
			</div>
		);
	}

	renderMopidyArtist(){
		if( !this.props.mopidy.artist ) return null
		var artist = this.props.mopidy.artist
		
		if( this.props.lastfm.artist.image && this.props.lastfm.artist.name == artist.name ) artist.images = this.props.lastfm.artist.image

		return (
			<div className="view artist-view">
				<Parallax images={ artist.images } />

				<div className="intro">
					<Thumbnail size="huge" images={ artist.images } />
					<h1>{ artist.name }</h1>
					<ul className="details">
						<li>{ artist.albums.length.toLocaleString() } albums</li>
						<li><FontAwesome name='folder' /> Local artist</li>
					</ul>
				</div>

				<h4 className="left-padding">Top tracks</h4>
				{ artist.tracks ? <TrackList tracks={ artist.tracks } /> : null }

				<h4 className="left-padding">Albums</h4>
				<section className="grid-wrapper no-top-padding">
					{ artist.albums ? <AlbumGrid albums={ artist.albums } /> : null }
				</section>
			</div>
		);
	}

	render(){
		var source = helpers.uriSource( this.props.params.uri );
		if( source == 'spotify' ) return this.renderSpotifyArtist()
		if( source == 'local' ) return this.renderMopidyArtist()
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
		lastfmActions: bindActionCreators(lastfmActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Artist)
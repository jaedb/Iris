
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'
let helpers = require('../helpers.js')

import TrackList from '../components/TrackList'
import Thumbnail from '../components/Thumbnail'
import Parallax from '../components/Parallax'
import ArtistSentence from '../components/ArtistSentence'
import ArtistGrid from '../components/ArtistGrid'
import Dater from '../components/Dater'
import LazyLoadListener from '../components/LazyLoadListener'

import * as mopidyActions from '../services/mopidy/actions'
import * as lastfmActions from '../services/lastfm/actions'
import * as spotifyActions from '../services/spotify/actions'

class Album extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.loadAlbum();
	}

	componentWillReceiveProps( nextProps ){
		if( nextProps.params.uri != this.props.params.uri ){
			this.loadAlbum( nextProps )
		}else if( !this.props.mopidy.connected && nextProps.mopidy.connected ){
			if( helpers.uriSource( this.props.params.uri ) == 'local' ){
				this.loadAlbum( nextProps )
			}
		}
	}

	loadAlbum( props = this.props ){
		var source = helpers.uriSource( props.params.uri );

		if( source == 'spotify' ){
			this.props.spotifyActions.getAlbum( props.params.uri );

		}else if( source == 'local' && props.mopidy.connected ){
			this.props.mopidyActions.getAlbum( props.params.uri );
		}
	}

	loadMore(){
		if( !this.props.spotify.album || !this.props.spotify.album.tracks.next ) return
		this.props.spotifyActions.getURL( this.props.spotify.album.tracks.next, 'SPOTIFY_ALBUM_LOADED_MORE' );
	}

	album(){
		var album = {
			name: false,
			tracks: {
				items: []
			},
			artists: [],
			images: []
		}
		
		switch( helpers.uriSource( this.props.params.uri ) ){

			case 'spotify':
				Object.assign(album, this.props.spotify.album)
				album.artists = this.props.spotify.artists
				break

			case 'local':
				Object.assign(album, this.props.mopidy.album)
				if( this.props.lastfm.album.image ) album.images = this.props.lastfm.album.image
				break
		}

		return album
	}

	render(){
		var album = this.album()

		return (
			<div className="view album-view">
				<div className="intro">
					<Thumbnail size="large" images={ album.images } />
					<ArtistGrid artists={ album.artists } />
					<ul className="details">
						<li>{ album.tracks.total } tracks, <Dater type="total-time" data={album.tracks.items} /></li>
						{ album.release_date ? <li>Released <Dater type="date" data={ album.release_date } /></li> : null }
						<li><FontAwesome name={helpers.sourceIcon( this.props.params.uri )} /> {helpers.uriSource( this.props.params.uri )} playlist</li>	
					</ul>
				</div>
				<div className="main">

					<div className="title">
						<h1>{ album.name }</h1>
						<h3><ArtistSentence artists={ album.artists } /></h3>
					</div>

					<section className="list-wrapper">
						<TrackList tracks={ album.tracks.items } />
						<LazyLoadListener loadMore={ () => this.loadMore() }/>
					</section>
					
				</div>
			</div>
		);
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

export default connect(mapStateToProps, mapDispatchToProps)(Album)
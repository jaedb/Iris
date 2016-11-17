
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
import * as spotifyActions from '../services/spotify/actions'

class Album extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.loadAlbum();
	}

	componentWillReceiveProps( nextProps ){

		// if our URI has changed, fetch new album
		if( nextProps.params.uri != this.props.params.uri ){
			this.loadAlbum( nextProps )

		// if mopidy has just connected AND we're a local album, go get
		}else if( !this.props.mopidy_connected && nextProps.mopidy_connected ){
			if( helpers.uriSource( this.props.params.uri ) == 'local' ){
				this.loadAlbum( nextProps )
			}
		}
	}

	loadAlbum( props = this.props ){
		switch( helpers.uriSource( props.params.uri ) ){

			case 'spotify':
				this.props.spotifyActions.getAlbum( props.params.uri );
				break;

			case 'local':
				if( props.mopidy_connected ) this.props.mopidyActions.getAlbum( props.params.uri );
				break;
		}
	}

	loadMore(){
		if( !this.props.album.tracks_more ) return
		this.props.spotifyActions.getURL( this.props.album.tracks_more, 'SPOTIFY_ALBUM_LOADED_MORE' );
	}

	play(){
		var tracks_uris = helpers.asURIs( this.props.album.tracks )
		this.props.mopidyActions.playURIs( tracks_uris )
	}

	follow(){
		this.props.spotifyActions.toggleAlbumInLibrary( this.props.params.uri, 'PUT' )
	}

	// TODO: Once unfollowing occurs, remove playlist from global playlists list
	unfollow(){
		this.props.spotifyActions.toggleAlbumInLibrary( this.props.params.uri, 'DELETE' )
	}

	renderExtraButtons(){
		switch( helpers.uriSource( this.props.params.uri ) ){
			case 'spotify':
				if( !this.props.spotify_authorized ) return null
				if( this.props.album.following ){
					return <button className="large tertiary" onClick={ e => this.unfollow() }>Remove from library</button>
				}
				return <button className="large tertiary" onClick={ e => this.follow() }>Add to library</button>
		}
	}

	render(){
		if( !this.props.album ) return null

		return (
			<div className="view album-view">
				<div className="intro">
					<Thumbnail size="large" images={ this.props.album.images } />
					<ArtistGrid artists={ this.props.album.artists } />

					<div className="actions">
						<button className="large primary" onClick={ e => this.play() }>Play</button>
						{ this.renderExtraButtons() }
					</div>

					<ul className="details">
						<li>{ this.props.album.tracks_total } tracks, <Dater type="total-time" data={this.props.album.tracks} /></li>
						{ this.props.album.release_date ? <li>Released <Dater type="date" data={ this.props.album.release_date } /></li> : null }
						<li><FontAwesome name={helpers.sourceIcon( this.props.params.uri )} /> {helpers.uriSource( this.props.params.uri )} playlist</li>	
					</ul>
				</div>
				<div className="main">

					<div className="title">
						<h1>{ this.props.album.name }</h1>
						<h3><ArtistSentence artists={ this.props.album.artists } /></h3>
					</div>

					<section className="list-wrapper">
						{ this.props.album.tracks ? <TrackList tracks={ this.props.album.tracks } /> : null }
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
	return {
		album: state.ui.album,
		spotify_authorized: state.spotify.authorized,
		mopidy_connected: state.mopidy.connected
	};
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Album)
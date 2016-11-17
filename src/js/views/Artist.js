
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
		}else if( !this.props.mopidy_connected && nextProps.mopidy_connected ){
			if( helpers.uriSource( this.props.params.uri ) == 'local' ){
				this.loadArtist( nextProps )
			}
		}
	}

	loadArtist( props = this.props ){
		var source = helpers.uriSource( props.params.uri );
		if( source == 'spotify' ){
			this.props.spotifyActions.getArtist( props.params.uri );
		}else if( source == 'local' && props.mopidy_connected ){
			this.props.mopidyActions.getArtist( props.params.uri );
		}
	}

	loadMore(){
		if( !this.props.artist.albums_more ) return
		this.props.spotifyActions.getURL( this.props.artist.albums_more, 'SPOTIFY_ARTIST_ALBUMS_LOADED_MORE' );
	}

	play(){
		alert('Yet to be implemented')
	}

	follow(){
		this.props.spotifyActions.toggleArtistInLibrary( this.props.params.uri, 'PUT' )
	}

	unfollow(){
		this.props.spotifyActions.toggleArtistInLibrary( this.props.params.uri, 'DELETE' )
	}

	renderExtraButtons(){
		switch( helpers.uriSource( this.props.params.uri ) ){
			case 'spotify':
				if( !this.props.spotify_authorized ) return null
				if( this.props.artist.following ){
					return <button className="large tertiary" onClick={ e => this.unfollow() }>Unfollow</button>
				}
				return <button className="large tertiary" onClick={ e => this.follow() }>Follow</button>
		}
	}

	render(){
		if( !this.props.artist ) return null
		var scheme = helpers.uriSource( this.props.params.uri );

		return (
			<div className="view artist-view">
				<Parallax images={this.props.artist.images} />

				<div className="intro">

					<Thumbnail size="huge" images={ this.props.artist.images } />

					<h1>{ this.props.artist.name }</h1>

					<div className="actions">
						<button className="large primary" onClick={ e => this.play() }>Start radio</button>
						{ this.renderExtraButtons() }
					</div>

					<ul className="details">
						{ this.props.artist.followers ? <li>{ this.props.artist.followers.total.toLocaleString() } followers</li> : null }
						{ this.props.artist.popularity ? <li>{ this.props.artist.popularity }% popularity</li> : null }
						{ scheme == 'local' ? <li>{ this.props.artist.albums.length.toLocaleString() } albums</li> : null }
						{ scheme == 'spotify' ? <li><FontAwesome name='spotify' /> Spotify artist</li> : null }
						{ scheme == 'local' ? <li><FontAwesome name='folder' /> Local artist</li> : null }
					</ul>

				</div>

				<div className="col w70">
					<h4 className="left-padding">Top tracks</h4>
					{ this.props.artist.tracks ? <TrackList tracks={ this.props.artist.tracks } /> : null }
				</div>

				<div className="col w5"></div>

				<div className="col w25">
					<h4>Related artists</h4>
					{ this.props.artist.related_artists ? <ArtistList artists={ this.props.artist.related_artists.slice(0,6) } /> : null }
				</div>

				<div className="cf"></div>

				<h4 className="left-padding">Albums</h4>
				<section className="grid-wrapper no-top-padding">
					{ this.props.artist.albums ? <AlbumGrid albums={ this.props.artist.albums } /> : null }
					<LazyLoadListener loadMore={ () => this.loadMore() }/>
				</section>
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
		artist: state.ui.artist,
		spotify_authorized: state.spotify.authorized,
		mopidy_connected: state.mopidy.connected
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		lastfmActions: bindActionCreators(lastfmActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Artist)
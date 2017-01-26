
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import TrackList from '../components/TrackList'
import Thumbnail from '../components/Thumbnail'
import Parallax from '../components/Parallax'
import ArtistSentence from '../components/ArtistSentence'
import ArtistGrid from '../components/ArtistGrid'
import FollowButton from '../components/FollowButton'
import Dater from '../components/Dater'
import LazyLoadListener from '../components/LazyLoadListener'
import SidebarToggleButton from '../components/SidebarToggleButton'

import * as helpers from '../helpers'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Album extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.loadAlbum() 
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
				if (props.album && props.album.tracks && props.album.artists_uris){
					console.info('Loading album from index')
				}else{
					this.props.spotifyActions.getAlbum( props.params.uri );
				}
				break;

			case 'local':
				if (props.mopidy_connected){
					if (props.album && props.album.tracks){
						console.info('Loading album from index')
					} else {
						this.props.mopidyActions.getAlbum( props.params.uri );
					}
				}
				break;
		}
	}

	loadMore(){
		this.props.spotifyActions.getURL( this.props.album.tracks_more, 'SPOTIFY_ALBUM_LOADED_MORE' );
	}

	play(){
		var tracks_uris = helpers.asURIs( this.props.album.tracks )
		this.props.mopidyActions.playURIs( tracks_uris )
	}

	render(){
		if( !this.props.album ) return null

		var artists = []
		if (this.props.album.artists_uris && this.props.artists){
			for (var i = 0; i < this.props.album.artists_uris.length; i++){
				var uri = this.props.album.artists_uris[i]
				if (this.props.artists.hasOwnProperty(uri)){
					artists.push(this.props.artists[uri])
				}
			}
		}

		return (
			<div className="view album-view">
		        <SidebarToggleButton />
				<div className="intro">
					<Thumbnail size="large" canZoom images={ this.props.album.images } />
					<ArtistGrid artists={artists} />

					<div className="actions">
						<button className="large primary" onClick={ e => this.play() }>Play</button>
						{ helpers.uriSource(this.props.params.uri) == 'spotify' ? <FollowButton uri={this.props.params.uri} addText="Add to library" removeText="Remove from library" is_following={this.props.album.is_following} /> : null }
					</div>

					<ul className="details">
						<li>
							{ this.props.album.tracks_total } tracks,&nbsp;
							{ this.props.album.tracks ? <Dater type="total-time" data={this.props.album.tracks} /> : null }
						</li>
						{ this.props.album.release_date ? <li>Released <Dater type="date" data={ this.props.album.release_date } /></li> : null }
						<li><FontAwesome name={helpers.sourceIcon( this.props.params.uri )} /> {helpers.uriSource( this.props.params.uri )} playlist</li>	
					</ul>
				</div>
				<div className="main">

					<div className="title">
						<h1>{ this.props.album.name }</h1>
						<h3><ArtistSentence artists={artists} /></h3>
					</div>

					<section className="list-wrapper">
						{ this.props.album.tracks ? <TrackList tracks={ this.props.album.tracks } /> : null }
						<LazyLoadListener enabled={this.props.album.tracks_more} loadMore={ () => this.loadMore() }/>
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
		artists: state.ui.artists,
		album: (state.ui.albums && typeof(state.ui.albums[ownProps.params.uri]) !== 'undefined' ? state.ui.albums[ownProps.params.uri] : false ),
		albums: state.ui.albums,
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
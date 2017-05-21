
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
import ContextMenuTrigger from '../components/ContextMenuTrigger'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Album extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.loadAlbum() 
	}

	handleContextMenu(e){
		e.preventDefault()
		var data = { uris: [this.props.params.uri] }
		this.props.uiActions.showContextMenu( e, data, 'album', 'click' )
	}

	componentWillReceiveProps( nextProps ){

		// if our URI has changed, fetch new album
		if( nextProps.params.uri != this.props.params.uri ){
			this.loadAlbum( nextProps )

		// if mopidy has just connected AND we're a local album, go get
		}else if( !this.props.mopidy_connected && nextProps.mopidy_connected ){
			if( helpers.uriSource( this.props.params.uri ) != 'spotify' ){
				this.loadAlbum( nextProps )
			}
		}
	}

	handleContextMenu(e){
		var data = {
			e: e,
			context: 'album',
			items: [this.props.album],
			uris: [this.props.params.uri]
		}
		this.props.uiActions.showContextMenu(data)
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

			default:
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
		this.props.mopidyActions.playURIs([this.props.params.uri], this.props.params.uri)
	}

	inLibrary(){
		return (this.props.library_albums && this.props.library_albums.indexOf(this.props.params.uri) > -1)
	}

	render(){
		if (helpers.isLoading(this.props.load_queue,['spotify_albums/'+helpers.getFromUri('albumid',this.props.params.uri)])){
			return (
				<div className="body-loader">
					<div className="loader"></div>
				</div>
			)
		}

		if (!this.props.album){
			return null
		}

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

				<Thumbnail size="large" canZoom images={ this.props.album.images } />

				<div className="title">
					<div className="source grey-text">
						<FontAwesome name={helpers.sourceIcon( this.props.params.uri )} />&nbsp;
						{helpers.uriSource( this.props.params.uri )}&nbsp;
						{this.props.album.album_type ? this.props.album.album_type : 'album'}
					</div>

					<h1>{ this.props.album.name }</h1>

					<ul className="details">
						{ artists.length > 0 ? <li><ArtistSentence artists={artists} /></li> : null }
						{ this.props.album.release_date ? <li><Dater type="date" data={ this.props.album.release_date } /></li> : null }
						<li>
							{ this.props.album.tracks_total ? this.props.album.tracks_total : '0' } tracks,&nbsp;
							{ this.props.album.tracks ? <Dater type="total-time" data={this.props.album.tracks} /> : '0 mins' }
						</li>
					</ul>
				</div>

				<div className="actions">
					<button className="primary" onClick={e => this.play()}>Play</button>
					{ helpers.uriSource(this.props.params.uri) == 'spotify' ? <FollowButton className="secondary" uri={this.props.params.uri} addText="Add to library" removeText="Remove from library" is_following={this.inLibrary()} /> : null }
					<ContextMenuTrigger onTrigger={e => this.handleContextMenu(e)} />
				</div>

				<section className="list-wrapper">
					{ this.props.album.tracks ? <TrackList className="album-track-list" tracks={ this.props.album.tracks } uri={this.props.params.uri} /> : null }
					<LazyLoadListener enabled={this.props.album.tracks_more} loadMore={ () => this.loadMore() }/>
				</section>

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
	return {
		load_queue: state.ui.load_queue,
		artists: state.ui.artists,
		album: (state.ui.albums && typeof(state.ui.albums[ownProps.params.uri]) !== 'undefined' ? state.ui.albums[ownProps.params.uri] : false ),
		albums: state.ui.albums,
		library_albums: state.ui.library_albums,
		spotify_authorized: state.spotify.authorized,
		mopidy_connected: state.mopidy.connected
	};
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Album)
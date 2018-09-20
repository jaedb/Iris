
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Header from '../components/Header'
import TrackList from '../components/TrackList'
import Thumbnail from '../components/Thumbnail'
import Parallax from '../components/Parallax'
import ArtistSentence from '../components/ArtistSentence'
import ArtistGrid from '../components/ArtistGrid'
import FollowButton from '../components/Fields/FollowButton'
import Dater from '../components/Dater'
import LazyLoadListener from '../components/LazyLoadListener'
import ContextMenuTrigger from '../components/ContextMenuTrigger'
import Icon from '../components/Icon'

import * as helpers from '../helpers'
import * as coreActions from '../services/core/actions'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Album extends React.Component{

	constructor(props){
		super(props);
	}

	componentDidMount(){
		this.setWindowTitle();
		this.props.coreActions.loadAlbum(this.props.params.uri);
	}

	handleContextMenu(e){
		e.preventDefault()
		var data = { uris: [this.props.params.uri] }
		this.props.uiActions.showContextMenu(e, data, 'album', 'click' )
	}

	componentWillReceiveProps(nextProps){

		// if our URI has changed, fetch new album
		if (nextProps.params.uri != this.props.params.uri){
			this.props.coreActions.loadAlbum(nextProps.params.uri);

		// if mopidy has just connected AND we're a local album, go get
		} else if (!this.props.mopidy_connected && nextProps.mopidy_connected){
			if (helpers.uriSource(nextProps.params.uri) != 'spotify'){
				this.props.coreActions.loadAlbum(nextProps.params.uri);
			}
		}

		if (!this.props.album && nextProps.album){
			this.setWindowTitle(nextProps.album);
		}
	}

	setWindowTitle(album = this.props.album){
		if (album){
			var artists = "";
			if (album.artists_uris && this.props.artists){
				for (var i = 0; i < album.artists_uris.length; i++){
					var uri = album.artists_uris[i]
					if (this.props.artists.hasOwnProperty(uri)){
						if (artists != ""){
							artists += ", ";
						}
						artists += this.props.artists[uri].name;
					}
				}
			}
			this.props.uiActions.setWindowTitle(album.name+" by "+artists+" (album)");
		} else{
			this.props.uiActions.setWindowTitle("Album");
		}
	}

	handleContextMenu(e){
		var data = {
			e: e,
			context: 'album',
			items: [this.props.album],
			uris: [this.props.params.uri]
		}
		this.props.uiActions.showContextMenu(data);
	}

	loadMore(){
		this.props.spotifyActions.getMore(
			this.props.album.tracks_more,
			{
				parent_type: 'album',
				parent_key: this.props.album.uri,
				records_type: 'track'
			}
		);
	}

	play(){
		this.props.mopidyActions.playURIs([this.props.params.uri], this.props.params.uri)
	}

	inLibrary(){
		var library = helpers.uriSource(this.props.params.uri)+'_library_albums'
		return (this.props[library] && this.props[library].indexOf(this.props.params.uri) > -1)
	}

	render(){
		if (!this.props.album){
			if (helpers.isLoading(this.props.load_queue,['spotify_albums/'+helpers.getFromUri('albumid',this.props.params.uri)])){
				return (
					<div className="body-loader loading">
						<div className="loader"></div>
					</div>
				)
			} else {
				return null;
			}
		}

		var album = helpers.collate(this.props.album, {tracks: this.props.tracks, artists: this.props.artists});

		if (!album.tracks_uris || (album.tracks_uris && !album.tracks) || (album.tracks_uris.length !== album.tracks.length)){
			var is_loading_tracks = true;
		} else {
			var is_loading_tracks = false;
		}

		return (
			<div className="view album-view content-wrapper">
				<div className="thumbnail-wrapper">
					<Thumbnail size="large" canZoom images={album.images} />
				</div>

				<div className="title">

					<h1>{album.name}</h1>

					<ul className="details">
						{!this.props.slim_mode ? <li className="has-tooltip"><Icon type="fontawesome" name={helpers.sourceIcon(album.uri )} /><span className="tooltip">{helpers.uriSource(this.props.params.uri )} {album.type ? album.type : 'album'}</span></li> : null}
						{!this.props.slim_mode && album.artists && album.artists.length > 0 ? <li><ArtistSentence artists={album.artists} /></li> : null}
						{album.release_date ? <li><Dater type="date" data={album.release_date} /></li> : null}
						<li>
							{album.tracks ? <span>{album.tracks.length} tracks, <Dater type="total-time" data={album.tracks} /></span> : '0 tracks, 0 mins' }
						</li>
					</ul>
				</div>

				<div className="actions">
					<button className="primary" onClick={e => this.play()}>Play</button>
					{ helpers.uriSource(this.props.params.uri) == 'spotify' ? <FollowButton className="secondary" uri={this.props.params.uri} addText="Add to library" removeText="Remove from library" is_following={this.inLibrary()} /> : null }
					<ContextMenuTrigger onTrigger={e => this.handleContextMenu(e)} />
				</div>

				<section className="list-wrapper">
					<TrackList className="album-track-list" tracks={album.tracks} uri={album.uri} />
					<LazyLoadListener
						loadKey={album.tracks_more}
						showLoader={is_loading_tracks}
						loadMore={() => this.loadMore()}
					/>
				</section>

			</div>
		)
	}
}

const mapStateToProps = (state, ownProps) => {
	var uri = ownProps.params.uri;
	return {
		slim_mode: state.ui.slim_mode,
		load_queue: state.ui.load_queue,
		tracks: state.core.tracks,
		artists: state.core.artists,
		album: (state.core.albums && state.core.albums[uri] !== undefined ? state.core.albums[uri] : false ),
		albums: state.core.albums,
		spotify_library_albums: state.spotify.library_albums,
		local_library_albums: state.mopidy.library_albums,
		spotify_authorized: state.spotify.authorization,
		mopidy_connected: state.mopidy.connected
	};
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Album)

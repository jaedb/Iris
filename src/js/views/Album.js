
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import ErrorMessage from '../components/ErrorMessage';
import Header from '../components/Header';
import TrackList from '../components/TrackList';
import Thumbnail from '../components/Thumbnail';
import Parallax from '../components/Parallax';
import ArtistSentence from '../components/ArtistSentence';
import ArtistGrid from '../components/ArtistGrid';
import FollowButton from '../components/Fields/FollowButton';
import NiceNumber from '../components/NiceNumber';
import Dater from '../components/Dater';
import LazyLoadListener from '../components/LazyLoadListener';
import ContextMenuTrigger from '../components/ContextMenuTrigger';
import Icon from '../components/Icon';

import * as helpers from '../helpers';
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as spotifyActions from '../services/spotify/actions';
import * as lastfmActions from '../services/lastfm/actions';

class Album extends React.Component{

	constructor(props){
		super(props);
	}

	componentDidMount(){
		this.setWindowTitle();
		this.props.coreActions.loadAlbum(this.props.uri);

		// We already have the album in our index, so it won't fire componentWillReceiveProps
		if (this.props.album){
			if (this.props.album.artists && this.props.album.wiki === undefined){
				this.props.lastfmActions.getAlbum(this.props.album.uri, this.props.album.artists[0].name, this.props.album.name);
			}
		}
	}

	handleContextMenu(e){
		e.preventDefault()
		var data = { uris: [this.props.uri] }
		this.props.uiActions.showContextMenu(e, data, 'album', 'click' )
	}

	componentWillReceiveProps(nextProps){

		// if our URI has changed, fetch new album
		if (nextProps.uri != this.props.uri){
			this.props.coreActions.loadAlbum(nextProps.uri);

		// if mopidy has just connected AND we're a local album, go get
		} else if (!this.props.mopidy_connected && nextProps.mopidy_connected){
			if (helpers.uriSource(nextProps.uri) != 'spotify'){
				this.props.coreActions.loadAlbum(nextProps.uri);
			}
		}

		// We have just received our full album or our album artists
		if ((!this.props.album && nextProps.album) || (!this.props.album.artists && nextProps.album.artists)){
			if (nextProps.album.wiki === undefined && nextProps.artists.length > 0){
				this.props.lastfmActions.getAlbum(nextProps.album.uri, nextProps.album.artists[0].name, nextProps.album.name);
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
			uris: [this.props.uri]
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
		this.props.mopidyActions.playURIs([this.props.uri], this.props.uri)
	}

	inLibrary(){
		var library = helpers.uriSource(this.props.uri)+'_library_albums'
		return (this.props[library] && this.props[library].indexOf(this.props.uri) > -1)
	}

	render(){
		if (!this.props.album){
			if (helpers.isLoading(this.props.load_queue,['spotify_albums/'+helpers.getFromUri('albumid',this.props.uri)])){
				return (
					<div className="body-loader loading">
						<div className="loader"></div>
					</div>
				)
			} else {
				return (
					<ErrorMessage type="not-found" title="Not found">
						<p>Could not find album with URI "{encodeURIComponent(this.props.uri)}"</p>
					</ErrorMessage>
				);
			}
		}

		var album = helpers.collate(this.props.album, {tracks: this.props.tracks, artists: this.props.artists});

		if (!album.tracks_uris || (album.tracks_uris && !album.tracks) || (album.tracks_uris.length !== album.tracks.length)){
			var is_loading_tracks = true;
		} else {
			var is_loading_tracks = false;
		}

		return (
			<div className="view album-view content-wrapper preserve-3d">

				<Parallax image={album.images ? album.images.huge : null} blur />

				<div className="thumbnail-wrapper">
					<Thumbnail size="large" canZoom images={album.images} />
				</div>

				<div className="title">

					<h1>{album.name}</h1>

					<ul className="details">
						{!this.props.slim_mode ? <li className="source"><Icon type="fontawesome" name={helpers.sourceIcon(album.uri )} /></li> : null}
						{album.artists && album.artists.length > 0 ? <li><ArtistSentence artists={album.artists} /></li> : null}
						{album.release_date ? <li><Dater type="date" data={album.release_date} /></li> : null}
						{album.tracks ? <li>{album.tracks.length} tracks</li> : null}
						{!this.props.slim_mode && album.tracks ? <li><Dater type="total-time" data={album.tracks} /></li> : null}
						{!this.props.slim_mode && album.play_count ? <li><NiceNumber value={album.play_count} /> plays</li> : null}
						{!this.props.slim_mode && album.listeners ? <li><NiceNumber value={album.listeners} /> listeners</li> : null}
					</ul>
				</div>

				<div className="actions">
					<button className="button button--primary" onClick={e => this.play()}>Play</button>
					{ helpers.uriSource(this.props.uri) == 'spotify' ? <FollowButton className="secondary" uri={this.props.uri} addText="Add to library" removeText="Remove from library" is_following={this.inLibrary()} /> : null }
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

				{album.wiki ? <section className="wiki">
					<h4 className="wiki__title">About</h4>
					<div className="wiki__text">
						<p>{album.wiki}</p><br />
						<div className="mid_grey-text">Published: { album.wiki_publish_date }</div>
					</div>
				</section> : null}

			</div>
		)
	}
}

const mapStateToProps = (state, ownProps) => {
	var uri = decodeURIComponent(ownProps.match.params.uri);
	return {
		uri: uri,
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
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		lastfmActions: bindActionCreators(lastfmActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Album)

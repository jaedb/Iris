
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import LazyLoadListener from '../components/LazyLoadListener'
import Header from '../components/Header'
import TrackList from '../components/TrackList'
import AlbumGrid from '../components/AlbumGrid'
import Thumbnail from '../components/Thumbnail'
import Parallax from '../components/Parallax'
import ArtistGrid from '../components/ArtistGrid'
import RelatedArtists from '../components/RelatedArtists'
import FollowButton from '../components/Fields/FollowButton'
import ContextMenuTrigger from '../components/ContextMenuTrigger'
import DropdownField from '../components/Fields/DropdownField'
import Icon from '../components/Icon'

import * as helpers from '../helpers'
import * as coreActions from '../services/core/actions'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as pusherActions from '../services/pusher/actions'
import * as lastfmActions from '../services/lastfm/actions'
import * as spotifyActions from '../services/spotify/actions'

class Artist extends React.Component{

	constructor(props){
		super(props)
	}

	componentDidMount(){
		this.setWindowTitle();
		this.loadArtist();
	}

	componentWillReceiveProps(nextProps){
		if (nextProps.params.uri != this.props.params.uri){
			this.loadArtist(nextProps);
		}else if (!this.props.mopidy_connected && nextProps.mopidy_connected){
			if (helpers.uriSource(this.props.params.uri ) != 'spotify'){
				this.loadArtist(nextProps);
			}
		}

		if (!this.props.artist && nextProps.artist){
			this.setWindowTitle(nextProps.artist);
		}

		if (this.props.params.uri !== nextProps.params.uri && nextProps.artist){
			this.setWindowTitle(nextProps.artist);
		}
	}

	setWindowTitle(artist = this.props.artist){
		if (artist){
			this.props.uiActions.setWindowTitle(artist.name+" (artist)");
		} else{
			this.props.uiActions.setWindowTitle("Artist");
		}
	}

	handleContextMenu(e){
		var data = {
			e: e,
			context: 'artist',
			items: [this.props.artist],
			uris: [this.props.params.uri]
		}
		this.props.uiActions.showContextMenu(data);
	}

	loadArtist(props = this.props){
		switch(helpers.uriSource(props.params.uri )){

			case 'spotify':
				if (props.artist && props.artist.albums_uris && props.artist.related_artists_uris){
					console.info('Loading spotify artist from index');
				} else {
					this.props.spotifyActions.getArtist(props.params.uri, true);
				}
				this.props.spotifyActions.following(props.params.uri);
				break

			default:
				if (props.mopidy_connected){
					if (props.artist && props.artist.images && props.artist.albums_uris){
						console.info('Loading local artist from index');
					} else {
						this.props.mopidyActions.getArtist(props.params.uri);
					}
				}
				break
		}
	}

	loadMore(){
		this.props.spotifyActions.getMore(
			this.props.artist.albums_more,
			{
				parent_type: 'artist',
				parent_key: this.props.params.uri,
				records_type: 'album'
			}
		);
	}

	inLibrary(){
		var library = helpers.uriSource(this.props.params.uri)+'_library_artists'
		return (this.props[library] && this.props[library].indexOf(this.props.params.uri) > -1)
	}

	setSort(value){
		var reverse = false
		if (this.props.sort == value ) reverse = !this.props.sort_reverse

		var data = {
			artist_albums_sort_reverse: reverse,
			artist_albums_sort: value
		}
		this.props.uiActions.set(data)
	}

	setFilter(value){
		this.props.uiActions.set({artist_albums_filter: value});
	}

	renderSubViewMenu(){
		return (
			<div className="sub-views">
				<Link className="option" activeClassName="active" to={global.baseURL+'artist/'+this.props.params.uri}><h4>Overview</h4></Link>
				{this.props.artist.related_artists_uris ? <Link className="option" activeClassName="active" to={global.baseURL+'artist/'+this.props.params.uri+'/related-artists'}><h4>Related artists</h4></Link> : null}
				<Link className="option" activeClassName="active" to={global.baseURL+'artist/'+this.props.params.uri+'/about'}><h4>About</h4></Link>
			</div>
		)
	}

	renderBody(){
		var scheme = helpers.uriSource(this.props.params.uri);

		var related_artists = []
		if (this.props.artist.related_artists_uris){
			for (var i = 0; i < this.props.artist.related_artists_uris.length; i++){
				var uri = this.props.artist.related_artists_uris[i]
				if (this.props.artists.hasOwnProperty(uri)){
					related_artists.push(this.props.artists[uri])
				}
			}
		}

		var albums = [];
		if (this.props.artist.albums_uris){
			var albums_uris = helpers.removeDuplicates(this.props.artist.albums_uris);
			for (var i = 0; i < albums_uris.length; i++){
				var uri = albums_uris[i];
				if (this.props.albums.hasOwnProperty(uri)){
					albums.push(this.props.albums[uri]);
				}
			}

			if (this.props.sort){
				albums = helpers.sortItems(albums, this.props.sort, this.props.sort_reverse);
			}

			if (this.props.filter){
				albums = helpers.applyFilter('album_type', this.props.filter, albums);
			}
		}

		switch (this.props.params.sub_view){

			case 'related-artists':
				return (
					<div className="body related-artists">
						<section className="grid-wrapper no-top-padding">
							<ArtistGrid artists={related_artists} />
						</section>
					</div>
				)

			case 'about':
				return (
					<div className="body about">

						<div className="col w40 tiles artist-stats">
							{this.props.artist.images ? <div className="tile thumbnail-wrapper"><Thumbnail size="huge" canZoom images={this.props.artist.images} /></div> : null}
							{this.props.artist.images_additional ? <div className="tile thumbnail-wrapper"><Thumbnail size="huge" canZoom images={this.props.artist.images_additional} /></div> : null}
							{this.props.artist.followers ? <div className="tile"><span className="content"><Icon type="fontawesome" name="users" />{this.props.artist.followers.total.toLocaleString() } followers</span></div> : null}
							{this.props.artist.popularity ? <div className="tile"><span className="content"><Icon type="fontawesome" name="fire" />{this.props.artist.popularity }% popularity</span></div> : null}
							{this.props.artist.listeners ? <div className="tile"><span className="content"><Icon type="fontawesome" name="headphones" />{ this.props.artist.listeners.toLocaleString() } listeners</span></div> : null }
						</div>

						<div className="col w60 biography">
							<section>
								{ this.props.artist.bio ? <div className="biography-text"><p>{this.props.artist.bio.content}</p><br />
								<div className="grey-text">Published: { this.props.artist.bio.published }</div>
								<div className="grey-text">Origin: <a href={ this.props.artist.bio.links.link.href } target="_blank">{ this.props.artist.bio.links.link.href }</a></div></div> : null }
							</section>
						</div>
					</div>
				)

			default:

				var sort_options = [
					{
						value: null,
						label: 'Default'
					},
					{
						value: 'name',
						label: 'Name'
					},
					{
						value: 'date',
						label: 'Date'
					},
					{
						value: 'tracks_total',
						label: 'Tracks'
					}
				];

				var filter_options = [
					{
						value: null,
						label: 'All'
					},
					{
						value: 'album',
						label: 'Albums'
					},
					{
						value: 'single',
						label: 'Singles'
					},
					{
						value: 'compilation',
						label: 'Compilations'
					}
				];

				var tracks = [];
				if (this.props.artist.tracks_uris && this.props.tracks){
					for (var i = 0; i < this.props.artist.tracks_uris.length; i++){
						var uri = this.props.artist.tracks_uris[i]
						if (this.props.tracks.hasOwnProperty(uri)){
							tracks.push(this.props.tracks[uri])
						}
					}
				}

				if (tracks.length <= 0 && helpers.isLoading(this.props.load_queue,['spotify_artists/'+helpers.getFromUri('artistid',this.props.params.uri)+'/top-tracks'])){
					var is_loading_tracks = true;
				} else {
					var is_loading_tracks = false;
				}

				return (
					<div className="body overview">
						<div className={"top-tracks col w"+(related_artists.length > 0 ? "70" : "100")}>
							<h4>Top tracks</h4>
							<div className="list-wrapper">
								<TrackList className="artist-track-list" uri={this.props.params.uri} tracks={tracks} />
								<LazyLoadListener forceLoader={is_loading_tracks} />
							</div>
						</div>

						<div className="col w5"></div>

						{related_artists.length > 0 ? <div className="col w25 related-artists"><h4>Related artists</h4><div className="list-wrapper"><RelatedArtists artists={related_artists.slice(0,6)} /></div><Link to={global.baseURL+'artist/'+this.props.params.uri+'/related-artists'} className="button grey">All related artists</Link></div> : null}

						<div className="cf"></div>

						<div className="albums">
							<h4>
								Albums
								<DropdownField
									icon="sort"
									name="Sort"
									value={this.props.sort}
									options={sort_options}
									selected_icon={this.props.sort ? (this.props.sort_reverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
									handleChange={value => {this.setSort(value); this.props.uiActions.hideContextMenu() }}
								/>
								<DropdownField
									icon="filter_list"
									name="Filter"
									value={this.props.filter}
									options={filter_options}
									handleChange={value => {this.setFilter(value); this.props.uiActions.hideContextMenu() }}
								/>
							</h4>

							<section className="grid-wrapper no-top-padding">
								<AlbumGrid albums={albums} />
								<LazyLoadListener loading={this.props.artist.albums_more} loadMore={() => this.loadMore()} />
							</section>
						</div>
					</div>
				)
		}
	}

	render(){
		var scheme = helpers.uriSource(this.props.params.uri);

		if (!this.props.artist){
			if (helpers.isLoading(this.props.load_queue,['spotify_artists/'+helpers.getFromUri('artistid',this.props.params.uri), 'lastfm_method=artist.getInfo'])){
				return (
					<div className="body-loader loading">
						<div className="loader"></div>
					</div>
				)
			} else {
				return null;
			}
		}

		if (this.props.artist && this.props.artist.images){
			var image = helpers.sizedImages(this.props.artist.images).huge;
		} else {
			var image = null;
		}

		var is_spotify = (scheme == 'spotify')

		if (this.props.artist.tracks_uris && this.props.artist.tracks_uris.length > 0){
			var uris_to_play = this.props.artist.tracks_uris
		} else {
			var uris_to_play = this.props.artist.albums_uris
		}

		return (
			<div className="view artist-view">
				<div className="intro">

					<Parallax image={image} theme={this.props.theme} />

					<div className="liner">
						<h1>{this.props.artist ? this.props.artist.name : null}</h1>
						<div className="actions">
							<button className="primary" onClick={e => this.props.mopidyActions.playURIs(uris_to_play, this.props.artist.uri)}>Play</button>
							{is_spotify ? <FollowButton uri={this.props.params.uri} removeText="Remove from library" addText="Add to library" is_following={this.inLibrary()} /> : null}
							<ContextMenuTrigger className="white" onTrigger={e => this.handleContextMenu(e)} />
						</div>
						{ this.renderSubViewMenu() }
					</div>
				</div>
				<div className="content-wrapper">
					{this.renderBody()}
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
	var uri = ownProps.params.uri;
	return {
		theme: state.ui.theme,
		slim_mode: state.ui.slim_mode,
		load_queue: state.ui.load_queue,
		artist: (state.core.artists[uri] !== undefined ? state.core.artists[uri] : false),
		tracks: state.core.tracks,
		artists: state.core.artists,
		spotify_library_artists: state.spotify.library_artists,
		local_library_artists: state.mopidy.library_artists,
		albums: (state.core.albums ? state.core.albums : []),
		filter: (state.ui.artist_albums_filter ? state.ui.artist_albums_filter : null),
		sort: (state.ui.artist_albums_sort ? state.ui.artist_albums_sort : null),
		sort_reverse: (state.ui.artist_albums_sort_reverse ? true : false),
		spotify_authorized: state.spotify.authorization,
		mopidy_connected: state.mopidy.connected
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch),
		lastfmActions: bindActionCreators(lastfmActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Artist)

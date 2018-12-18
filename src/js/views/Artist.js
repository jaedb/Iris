
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Link from '../components/Link';
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
		this.props.coreActions.loadArtist(this.props.params.uri);
	}

	componentWillReceiveProps(nextProps){

		if (nextProps.params.uri != this.props.params.uri){
			this.props.coreActions.loadArtist(nextProps.params.uri);

		} else if (!this.props.mopidy_connected && nextProps.mopidy_connected){
			if (helpers.uriSource(this.props.params.uri ) != 'spotify'){
				this.props.coreActions.loadArtist(nextProps.params.uri);
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
			<div className="sub-views" id="sub-views-menu">
				<Link className="option" activeClassName="active" to={global.baseURL+'artist/'+this.props.params.uri} scrollTo="sub-views-menu"><h4>Overview</h4></Link>
				{this.props.artist.related_artists_uris ? <Link className="option" activeClassName="active" to={global.baseURL+'artist/'+this.props.params.uri+'/related-artists'} scrollTo="sub-views-menu"><h4>Related artists</h4></Link> : null}
				<Link className="option" activeClassName="active" to={global.baseURL+'artist/'+this.props.params.uri+'/about'} scrollTo="sub-views-menu"><h4>About</h4></Link>
			</div>
		)
	}

	renderBody(){
		var scheme = helpers.uriSource(this.props.params.uri);
		var artist = helpers.collate(
			this.props.artist, 
			{
				artists: this.props.artists,
				albums: this.props.albums,
				tracks: this.props.tracks
			}
		);

		if (this.props.sort && artist.albums){
			artist.albums = helpers.sortItems(artist.albums, this.props.sort, this.props.sort_reverse);
		}

		if (this.props.filter && artist.albums){
			artist.albums = helpers.applyFilter('type', this.props.filter, artist.albums);
		}

		switch (this.props.params.sub_view){

			case 'related-artists':
				return (
					<div className="body related-artists">
						<section className="grid-wrapper no-top-padding">
							<ArtistGrid artists={artist.related_artists} />
						</section>
					</div>
				)

			case 'about':

				var thumbnails = [];
				if (artist.images && artist.images.length > 0){
					for (var i = 0; i < artist.images.length; i++){
						thumbnails.push(
							<div className="tile thumbnail-wrapper" key={i}>
								<Thumbnail size="huge" canZoom images={artist.images[i]} />
							</div>
						);
					}
				}

				return (
					<div className="body about">

						<div className="col col--w40 tiles artist-stats">
							{thumbnails}
							{artist.followers ? <div className="tile"><span className="content"><Icon type="fontawesome" name="users" />{artist.followers.toLocaleString()} followers</span></div> : null}
							{artist.popularity ? <div className="tile"><span className="content"><Icon type="fontawesome" name="fire" />{artist.popularity }% popularity</span></div> : null}
							{artist.listeners ? <div className="tile"><span className="content"><Icon type="fontawesome" name="headphones" />{ artist.listeners.toLocaleString() } listeners</span></div> : null }
						</div>

						<div className="col col--w60 biography">
							<section>
								{ artist.biography ? <div className="biography-text"><p>{artist.biography}</p><br />
								<div className="mid_grey-text">Published: { artist.biography_publish_date }</div>
								<div className="mid_grey-text">Origin: <a href={ artist.biography_link } target="_blank">{ artist.biography_link }</a></div></div> : null }
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

				if (!artist.tracks_uris || (artist.tracks_uris && !artist.tracks) || (artist.tracks_uris.length !== artist.tracks.length)){
					var is_loading_tracks = true;
				} else {
					var is_loading_tracks = false;
				}

				return (
					<div className="body overview">
						<div className={"top-tracks col col--w"+(artist.related_artists && artist.related_artists.length > 0 ? "70" : "100")}>
							{artist.tracks ? <h4>Top tracks</h4> : null}
							<div className="list-wrapper">
								<TrackList className="artist-track-list" uri={artist.uri} tracks={artist.tracks} />
								<LazyLoadListener showLoader={is_loading_tracks} />
							</div>
						</div>

						<div className="col col--w5"></div>

						{artist.related_artists && artist.related_artists.length > 0 ? <div className="col col--w25 related-artists"><h4>Related artists</h4><div className="list-wrapper"><RelatedArtists artists={artist.related_artists.slice(0,6)} /></div><Link to={global.baseURL+'artist/'+artist.uri+'/related-artists'} scrollTo="sub-views-menu" className="button button--default">All related artists</Link></div> : null}

						<div className="cf"></div>

						{artist.albums ? <div className="albums">
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
								<AlbumGrid albums={artist.albums} />
								<LazyLoadListener
									loadKey={artist.albums_more} 
									showLoader={artist.albums_more} 
									loadMore={() => this.loadMore()} 
								/>
							</section>
						</div> : null}
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

		if (this.props.artist && this.props.artist.images && this.props.artist.images.length > 0){
			var image = this.props.artist.images[0].huge;
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
			<div className="view artist-view preserve-3d">
				<div className="intro preserve-3d">

					<Parallax image={image} boxed />

					<div className="liner">
						<h1>{this.props.artist ? this.props.artist.name : null}</h1>
						<div className="actions">
							<button className="button button--primary" onClick={e => this.props.mopidyActions.playURIs(uris_to_play, this.props.artist.uri)}>Play</button>
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

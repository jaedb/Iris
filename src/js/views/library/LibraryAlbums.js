
import React, { PropTypes } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Link from '../../components/Link';

import AlbumGrid from '../../components/AlbumGrid';
import List from '../../components/List';
import Header from '../../components/Header';
import Thumbnail from '../../components/Thumbnail';
import TrackList from '../../components/TrackList';
import ArtistSentence from '../../components/ArtistSentence';
import DropdownField from '../../components/Fields/DropdownField';
import FilterField from '../../components/Fields/FilterField';
import LazyLoadListener from '../../components/LazyLoadListener';
import Icon from '../../components/Icon';

import * as helpers from '../../helpers';
import * as coreActions from '../../services/core/actions';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as googleActions from '../../services/google/actions';
import * as spotifyActions from '../../services/spotify/actions';

class LibraryAlbums extends React.Component{

	constructor(props){
		super(props)

		this.state = {
			filter: '',
			limit: 50,
			per_page: 50
		}
	}

	componentDidMount(){
		this.props.uiActions.setWindowTitle("Albums");

		if (this.props.mopidy_connected && this.props.mopidy_library_albums_status != 'finished' && this.props.mopidy_library_albums_status != 'started' && (this.props.source == 'all' || this.props.source == 'local')){
			this.props.mopidyActions.getLibraryAlbums();
		}

		if (this.props.google_enabled && this.props.google_library_albums_status != 'finished' && this.props.google_library_albums_status != 'started' && (this.props.source == 'all' || this.props.source == 'google')){
			this.props.googleActions.getLibraryAlbums();
		}

		if (this.props.spotify_enabled && this.props.spotify_library_albums_status != 'finished' && this.props.spotify_library_albums_status != 'started' && (this.props.source == 'all' || this.props.source == 'spotify')){
			this.props.spotifyActions.getLibraryAlbums();
		}
	}

	componentWillReceiveProps(newProps){
		if (newProps.mopidy_connected && (newProps.source == 'all' || newProps.source == 'local')){

			// We've just connected
			if (!this.props.mopidy_connected){
				this.props.mopidyActions.getLibraryAlbums();
			}		

			// Filter changed, but we haven't got this provider's library yet
			if (this.props.source != 'all' && this.props.source != 'local' && newProps.mopidy_library_albums_status != 'finished' && newProps.mopidy_library_albums_status != 'started'){
				this.props.mopidyActions.getLibraryAlbums();
			}			
		}

		if (newProps.google_enabled && (newProps.source == 'all' || newProps.source == 'google')){

			// We've just been enabled (or detected as such)
			if (!this.props.google_enabled){
				this.props.googleActions.getLibraryAlbums();
			}		

			// Filter changed, but we haven't got this provider's library yet
			if (this.props.source != 'all' && this.props.source != 'google' && newProps.google_library_albums_status != 'finished' && newProps.google_library_albums_status != 'started'){
				this.props.googleActions.getLibraryAlbums();
			}			
		}

		if (newProps.spotify_enabled && (newProps.source == 'all' || newProps.source == 'spotify')){	

			// Filter changed, but we haven't got this provider's library yet
			if (newProps.spotify_library_albums_status != 'finished' && newProps.spotify_library_albums_status != 'started'){
				this.props.spotifyActions.getLibraryAlbums();
			}			
		}
	}

	handleContextMenu(e,item){
		var data = {
			e: e,
			context: 'album',
			uris: [item.uri],
			items: [item]
		}
		this.props.uiActions.showContextMenu(data)
	}

	moreURIsToLoad(){
		var uris = []
		if (this.props.albums && this.props.library_albums){
			for (var i = 0; i < this.props.library_albums.length; i++){
				var uri = this.props.library_albums[i]
				if (!this.props.albums.hasOwnProperty(uri) && helpers.uriSource(uri) == 'local'){
					uris.push(uri)
				}

				// limit each lookup to 50 URIs
				if (uris.length >= 50) break
			}
		}

		return uris
	}

	setSort(value){
		var reverse = false
		if (this.props.sort == value ) reverse = !this.props.sort_reverse

		var data = {
			library_albums_sort_reverse: reverse,
			library_albums_sort: value
		}
		this.props.uiActions.set(data)
	}

	renderView(){
		var albums = []

		// Spotify library items
		if (this.props.spotify_library_albums && (this.props.source == 'all' || this.props.source == 'spotify')){
			for (var uri of this.props.spotify_library_albums){
				if (this.props.albums.hasOwnProperty(uri)){
					albums.push(this.props.albums[uri]);
				}
			}
		}

		// Mopidy library items
		if (this.props.mopidy_library_albums && (this.props.source == 'all' || this.props.source == 'local')){
			for (var uri of this.props.mopidy_library_albums){

				// Construct item placeholder. This is used as Mopidy needs to 
				// lookup ref objects to get the full object which can take some time
				var source = helpers.uriSource(uri);
				var album = {
					uri: uri,
					source: source
				}

				if (this.props.albums.hasOwnProperty(uri)){
					album = this.props.albums[uri]
				}

				albums.push(album)
			}
		}

		// Google library items
		if (this.props.google_library_albums && (this.props.source == 'all' || this.props.source == 'google')){
			for (var uri of this.props.google_library_albums){

				// Construct item placeholder. This is used as Mopidy needs to 
				// lookup ref objects to get the full object which can take some time
				var source = helpers.uriSource(uri);
				var album = {
					uri: uri,
					source: source
				}

				if (this.props.albums.hasOwnProperty(uri)){
					album = this.props.albums[uri]
				}

				albums.push(album);
			}
		}

		if (this.props.sort){
			albums = helpers.sortItems(albums, this.props.sort, this.props.sort_reverse);
		}

		if (this.state.filter && this.state.filter !== ''){
			albums = helpers.applyFilter('name', this.state.filter, albums)
		}

		// Apply our lazy-load-rendering
		var total_albums = albums.length;
		albums = albums.slice(0, this.state.limit);

		if (this.props.view == 'list'){
			var columns = [
				{
					label: 'Name',
					name: 'name'
				},
				{
					label: 'Artists',
					name: 'artists'
				},
				{
					label: 'Added',
					name: 'added_at'
				},
				{
					label: 'Tracks',
					name: 'tracks_total'
				},
				{
					label: 'Source',
					name: 'source'
				}
			]
			return (
				<section className="content-wrapper">
					<List 
						handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
						rows={albums}
						thumbnail={true}
						details={['artists','tracks_uris.length']}
						right_column={['added_at']}
						className="albums"
						link_prefix={global.baseURL+"album/"} />
					<LazyLoadListener
						loadKey={this.state.limit}
						showLoader={this.state.limit < total_albums}
						loadMore={() => this.setState({limit: this.state.limit + this.state.per_page})}
					/>
				</section>
			)
		} else {
			return (
				<section className="content-wrapper">
					<AlbumGrid 
						handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
						albums={albums} />
					<LazyLoadListener
						loadKey={this.state.limit}
						showLoader={this.state.limit < total_albums}
						loadMore={() => this.setState({limit: this.state.limit + this.state.per_page})}
					/>
				</section>
			)
		}
	}

	render(){
		var source_options = [
			{
				value: 'all',
				label: 'All'
			},
			{
				value: 'local',
				label: 'Local'
			}
		];

		if (this.props.spotify_enabled){
			source_options.push({
				value: 'spotify',
				label: 'Spotify'
			});
		}

		if (this.props.google_enabled){
			source_options.push({
				value: 'google',
				label: 'Google'
			});
		}

		var view_options = [
			{
				value: 'thumbnails',
				label: 'Thumbnails'
			},
			{
				value: 'list',
				label: 'List'
			}
		]

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
				value: 'artists',
				label: 'Artist'
			},
			{
				value: 'added_at',
				label: 'Added'
			},
			{
				value: 'tracks_total',
				label: 'Tracks'
			},
			{
				value: 'source',
				label: 'Source'
			}
		]

		var options = (
			<span>
				<FilterField 
					initialValue={this.state.filter}
					handleChange={value => this.setState({filter: value, limit: this.state.per_page})}
				/>
				<DropdownField
					icon="sort" 
					name="Sort"
					value={this.props.sort} 
					options={sort_options} 
					selected_icon={this.props.sort ? (this.props.sort_reverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null} 
					handleChange={val => {this.setSort(val); this.props.uiActions.hideContextMenu() }}
				/>
				<DropdownField
					icon="visibility"
					name="View"
					value={this.props.view}
					options={view_options}
					handleChange={val => {this.props.uiActions.set({ library_albums_view: val }); this.props.uiActions.hideContextMenu() }}
				/>
				<DropdownField
					icon="cloud"
					name="Source"
					value={this.props.source}
					options={source_options}
					handleChange={val => {this.props.uiActions.set({ library_albums_source: val}); this.props.uiActions.hideContextMenu() }}
				/>
			</span>
		)

		return (
			<div className="view library-albums-view">
				<Header options={options} uiActions={this.props.uiActions}>
					<Icon name="album" type="material" />
					My albums
				</Header>
				{this.renderView()}
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
		mopidy_connected: state.mopidy.connected,
		mopidy_uri_schemes: state.mopidy.uri_schemes,
		load_queue: state.ui.load_queue,
		albums: state.core.albums,
		mopidy_library_albums: state.mopidy.library_albums,
		mopidy_library_albums_status: (state.ui.processes.MOPIDY_LIBRARY_ALBUMS_PROCESSOR !== undefined ? state.ui.processes.MOPIDY_LIBRARY_ALBUMS_PROCESSOR.status : null),
		google_enabled: state.google.enabled,
		google_library_albums: state.google.library_albums,
		google_library_albums_status: (state.ui.processes.GOOGLE_LIBRARY_ALBUMS_PROCESSOR !== undefined ? state.ui.processes.GOOGLE_LIBRARY_ALBUMS_PROCESSOR.status : null),
		spotify_enabled: state.spotify.enabled,
		spotify_library_albums: state.spotify.library_albums,
		spotify_library_albums_status: (state.ui.processes.SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR !== undefined ? state.ui.processes.SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR.status : null),
		view: state.ui.library_albums_view,
		source: (state.ui.library_albums_source ? state.ui.library_albums_source : 'all'),
		sort: (state.ui.library_albums_sort ? state.ui.library_albums_sort : null),
		sort_reverse: (state.ui.library_albums_sort_reverse ? state.ui.library_albums_sort_reverse : false)
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		googleActions: bindActionCreators(googleActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryAlbums)
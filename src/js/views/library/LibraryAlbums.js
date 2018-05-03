
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import AlbumGrid from '../../components/AlbumGrid'
import List from '../../components/List'
import Header from '../../components/Header'
import Thumbnail from '../../components/Thumbnail'
import TrackList from '../../components/TrackList'
import ArtistSentence from '../../components/ArtistSentence'
import DropdownField from '../../components/Fields/DropdownField'
import FilterField from '../../components/Fields/FilterField'
import LazyLoadListener from '../../components/LazyLoadListener'

import * as helpers from '../../helpers'
import * as coreActions from '../../services/core/actions'
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

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
		if (this.props.mopidy_library_albums_status != 'finished' && this.props.mopidy_library_albums_status != 'started' && this.props.mopidy_connected && (this.props.source == 'all' || this.props.source == 'local')){
			this.props.mopidyActions.getLibraryAlbums()
		}

		if (this.props.spotify_library_albums_status != 'finished' && this.props.spotify_library_albums_status != 'started' && (this.props.source == 'all' || this.props.source == 'spotify')){
			this.props.spotifyActions.getLibraryAlbums()
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

		if (newProps.mopidy_uri_schemes.includes('spotify:') && (newProps.source == 'all' || newProps.source == 'spotify')){	

			// Filter changed, but we haven't got this provider's library yet
			if (this.props.source != 'all' && this.props.source != 'spotify' && newProps.spotify_library_albums_status != 'finished' && newProps.spotify_library_albums_status != 'started'){
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
			for (var i = 0; i < this.props.spotify_library_albums.length; i++){
				var uri = this.props.spotify_library_albums[i]
				if (this.props.albums.hasOwnProperty(uri)){
					albums.push(this.props.albums[uri])
				}
			}
		}

		// Mopidy library items
		if (this.props.mopidy_library_albums && (this.props.source == 'all' || this.props.source == 'local')){
			for (var i = 0; i < this.props.mopidy_library_albums.length; i++){

				// Construct item placeholder. This is used as Mopidy needs to 
				// lookup ref objects to get the full object which can take some time
				var uri = this.props.mopidy_library_albums[i]
				var source = helpers.uriSource(uri)
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

		albums = helpers.sortItems(albums, this.props.sort, this.props.sort_reverse)

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
						columns={columns} 
						className="album-list"
						link_prefix={global.baseURL+"album/"} />
					<LazyLoadListener loading={this.state.limit < total_albums} loadMore={() => this.setState({limit: this.state.limit + this.state.per_page})} />
				</section>
			)
		} else {
			return (
				<section className="content-wrapper">
					<AlbumGrid 
						handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
						albums={albums} />
					<LazyLoadListener loading={this.state.limit < total_albums} loadMore={() => this.setState({limit: this.state.limit + this.state.per_page})} />
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

		if (this.props.mopidy_uri_schemes.includes('spotify:')){
			source_options.push({
				value: 'spotify',
				label: 'Spotify'
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
				<FilterField handleChange={value => this.setState({filter: value, limit: this.state.per_page})} />
				<DropdownField
					icon="sort" 
					name="Sort"
					value={this.props.sort} 
					options={sort_options} 
					reverse={this.props.sort_reverse} 
					handleChange={val => {this.setSort(val); this.props.uiActions.hideContextMenu() }}
				/>
				<DropdownField
					icon="eye"
					name="View"
					value={this.props.view}
					options={view_options}
					handleChange={val => {this.props.uiActions.set({ library_albums_view: val }); this.props.uiActions.hideContextMenu() }}
				/>
				<DropdownField
					icon="database"
					name="Source"
					value={this.props.source}
					options={source_options}
					handleChange={val => {this.props.uiActions.set({ library_albums_source: val}); this.props.uiActions.hideContextMenu() }}
				/>
			</span>
		)

		return (
			<div className="view library-albums-view">
				<Header icon="cd" title="My albums" options={options} uiActions={this.props.uiActions} />
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
		spotify_library_albums: state.spotify.library_albums,
		spotify_library_albums_status: (state.ui.processes.SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR !== undefined ? state.ui.processes.SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR.status : null),
		view: state.ui.library_albums_view,
		source: (state.ui.library_albums_source ? state.ui.library_albums_source : 'all'),
		sort: (state.ui.library_albums_sort ? state.ui.library_albums_sort : 'name'),
		sort_reverse: (state.ui.library_albums_sort_reverse ? true : false)
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryAlbums)
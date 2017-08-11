
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
import DropdownField from '../../components/DropdownField'
import LazyLoadListener from '../../components/LazyLoadListener'

import * as helpers from '../../helpers'
import * as coreActions from '../../services/core/actions'
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryAlbums extends React.Component{

	constructor(props) {
		super(props)
	}

	componentDidMount(){
		if (!this.props.library_albums){
			this.props.coreActions.getLibraryAlbums()
		}
	}

	componentWillReceiveProps(newProps){
		if (!this.props.spotify_connected && newProps.spotify_connected){
			this.props.spotifyActions.getLibraryAlbums()
		}
		if (!this.props.mopidy_connected && newProps.mopidy_connected){
			this.props.mopidyActions.getLibraryAlbums()
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

	loadMoreSpotify(){
		this.props.spotifyActions.getURL( this.props.library_albums_more, 'SPOTIFY_LIBRARY_ALBUMS_LOADED' );
	}

	loadMoreMopidy(){
		var uris = this.moreURIsToLoad()
		this.props.mopidyActions.getAlbums(uris)
	}

	setSort(value){
		var reverse = false
		if( this.props.sort == value ) reverse = !this.props.sort_reverse

		var data = {
			library_albums_sort_reverse: reverse,
			library_albums_sort: value
		}
		this.props.uiActions.set(data)
	}

	renderView(albums){
		if (!albums || albums.length <= 0) return null

		if( this.props.view == 'list' ){
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
				<List 
					handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
					rows={albums} 
					columns={columns} 
					className="album-list"
					link_prefix={global.baseURL+"album/"} />
			)
		}else if( this.props.view == 'detail' ){
			return (
				<div>
					{
						albums.map( album => {
							return (
								<div className="album" key={album.uri}>
									<Link to={global.baseURL+'album/'+album.uri}>
										<Thumbnail size="medium" images={album.images} />
									</Link>
									<div className="detail">
										<Link to={global.baseURL+'album/'+album.uri}>
											<h2>{ album.name }</h2>
										</Link>
										<h3><ArtistSentence className="grey-text" artists={album.artists} /></h3>
									</div>
									<div className="list-wrapper">
										<TrackList tracks={album.tracks} />
									</div>
								</div>
							)
						})
					}
				</div>			
			)
		}else{
			return (
				<AlbumGrid 
					handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
					albums={albums} />
			)
		}
	}

	render(){
		var albums = []
		if (this.props.library_albums && this.props.albums){
			for (var i = 0; i < this.props.library_albums.length; i++){

				var uri = this.props.library_albums[i]
				var source = helpers.uriSource(uri)
				var album = {
					uri: uri,
					source: source
				}

				if (this.props.albums.hasOwnProperty(uri)){
					album = this.props.albums[uri]
				}

				switch (this.props.filter){

					case 'spotify':
						if (source == 'spotify'){
							albums.push(album)
						}
						break

					case 'local':
						if (source == 'local'){
							albums.push(album)
						}
						break

					default:
						albums.push(album)
						break
				}
			}

			if( this.props.sort ){
				albums = helpers.sortItems(albums, this.props.sort, this.props.sort_reverse)
			}
		}

		var filter_options = [
			{
				value: 'all',
				label: 'All'
			},
			{
				value: 'local',
				label: 'Local'
			},
			{
				value: 'spotify',
				label: 'Spotify'
			}
		]

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
				<DropdownField icon="filter" name="Filter" value={this.props.filter} options={filter_options} handleChange={val => {this.props.uiActions.set({ library_albums_filter: val}); this.props.uiActions.hideContextMenu() }} />
				<DropdownField icon="sort" name="Sort" value={this.props.sort} options={sort_options} reverse={this.props.sort_reverse} handleChange={val => {this.setSort(val); this.props.uiActions.hideContextMenu() }} />
				<DropdownField icon="eye" name="View" value={this.props.view} options={view_options} handleChange={val => {this.props.uiActions.set({ library_albums_view: val }); this.props.uiActions.hideContextMenu() }} />
			</span>
		)

		return (
			<div className="view library-albums-view">

				<Header icon="cd" title="My albums" options={options} uiActions={this.props.uiActions} />

				<section className="content-wrapper">
					{ this.renderView(albums) }
					<LazyLoadListener 
						loading={this.props.library_albums_more && (this.props.filter == 'all' || this.props.filter == 'spotify')} 
						loadMore={() => this.loadMoreSpotify()}
					/>
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
		mopidy_connected: state.mopidy.connected,
		spotify_connected: state.spotify.connected,
		load_queue: state.ui.load_queue,
		albums: state.core.albums,
		library_albums: state.core.library_albums,
		view: state.ui.library_albums_view,
		filter: (state.ui.library_albums_filter ? state.ui.library_albums_filter : 'all'),
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
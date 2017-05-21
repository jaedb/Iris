
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
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryAlbums extends React.Component{

	constructor(props) {
		super(props)
	}

	componentDidMount(){
		if (!this.props.library_albums_started){
			this.props.spotifyActions.getLibraryAlbums();
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

	loadMore(){
		this.props.spotifyActions.getURL( this.props.library_albums_more, 'SPOTIFY_LIBRARY_ALBUMS_LOADED' );
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
					label: 'Released',
					name: 'release_date'
				},
				{
					label: 'Tracks',
					name: 'tracks_total'
				}
			]
			return (
				<section className="list-wrapper">
					<List 
						handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
						rows={albums} 
						columns={columns} 
						className="album-list"
						link_prefix={global.baseURL+"album/"} />
				</section>
			)
		}else if( this.props.view == 'thumbnails' ){
			return (
				<section className="grid-wrapper">
					<AlbumGrid 
						handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
						albums={albums} />
				</section>			
			)
		}else{
			return (
				<section className="grid-wrapper albums-detail-subview">
					{
						albums.map( album => {
							return (
								<div className="album" key={album.uri}>
									<Link to={global.baseURL+'album/'+album.uri}>
										<Thumbnail size="medium" images={album.images} />
									</Link>
									<div className="detail">
										<Link to={global.baseURL+'album/'+album.uri}>
											<h3>{ album.name }</h3>
										</Link>
										<h4><ArtistSentence className="grey-text" artists={album.artists} /></h4>
									</div>
									<div className="list-wrapper">
										<TrackList tracks={album.tracks} />
									</div>
								</div>
							)
						})
					}
				</section>			
			)
		}
	}

	render(){
		if (helpers.isLoading(this.props.load_queue,['spotify_me/albums'])){
			return (
				<div className="view library-albums-view">
					<Header icon="cd" title="My albums" />
					<div className="body-loader">
						<div className="loader"></div>
					</div>
				</div>
			)
		}

		var albums = []

		if (this.props.library_albums && this.props.albums){
			for (var i = 0; i < this.props.library_albums.length; i++){
				var uri = this.props.library_albums[i]
				if (this.props.albums.hasOwnProperty(uri)){
					albums.push(this.props.albums[uri])
				}
			}

			if( this.props.sort ){
				albums = helpers.sortItems(albums, this.props.sort, this.props.sort_reverse)
			}
		}

		var view_options = [
			{
				value: 'detail',
				label: 'Detail'
			},
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
				value: 'release_date',
				label: 'Released'
			},
			{
				value: 'tracks_total',
				label: 'Tracks'
			}
		]

		var options = (
			<span>
				<DropdownField icon="sort" name="Sort" value={this.props.sort} options={sort_options} reverse={this.props.sort_reverse} handleChange={val => {this.setSort(val); this.props.uiActions.hideContextMenu() }} />
				<DropdownField icon="eye" name="View" value={this.props.view} options={view_options} handleChange={val => {this.props.uiActions.set({ library_albums_view: val }); this.props.uiActions.hideContextMenu() }} />
			</span>
		)

		return (
			<div className="view library-albums-view">
				<Header icon="cd" title="My albums" options={options} uiActions={this.props.uiActions} />
				{ this.renderView(albums) }
				<LazyLoadListener enabled={this.props.library_albums_more} loadMore={ () => this.loadMore() }/>
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
		load_queue: state.ui.load_queue,
		view: state.ui.library_albums_view,
		albums: state.ui.albums,
		sort: (state.ui.library_albums_sort ? state.ui.library_albums_sort : 'name'),
		sort_reverse: (state.ui.library_albums_sort_reverse ? true : false),
		library_albums: state.ui.library_albums,
		library_albums_started: state.ui.library_albums_started,
		library_albums_more: state.ui.library_albums_more
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryAlbums)
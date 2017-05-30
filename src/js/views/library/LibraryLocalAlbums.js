
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import AlbumGrid from '../../components/AlbumGrid'
import Header from '../../components/Header'
import DropdownField from '../../components/DropdownField'
import List from '../../components/List'
import LazyLoadListener from '../../components/LazyLoadListener'

import * as helpers from '../../helpers'
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryLocalAlbums extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.loadAlbums()
	}

	componentWillReceiveProps( nextProps ){
		if (!this.props.mopidy_connected && nextProps.mopidy_connected){
			this.loadAlbums(nextProps);
		}
	}

	loadAlbums(props = this.props){
		if (props.mopidy_connected && !this.props.local_albums){
			this.props.mopidyActions.getLibraryAlbums();
		}
	}

	moreURIsToLoad(){
		var uris = []
		if (this.props.albums && this.props.local_albums){
			for (var i = 0; i < this.props.local_albums.length; i++){
				var uri = this.props.local_albums[i]
				if (!this.props.albums.hasOwnProperty(uri)){
					uris.push(uri)
				}

				// limit each lookup to 50 URIs
				if (uris.length >= 50) break
			}
		}

		return uris
	}

	loadMore(){
		var uris = this.moreURIsToLoad()
		this.props.mopidyActions.getAlbums(uris)
	}

	setSort(value){
		var reverse = false
		if( this.props.sort == value ) reverse = !this.props.sort_reverse

		var data = {
			library_local_albums_sort_reverse: reverse,
			library_local_albums_sort: value
		}
		this.props.uiActions.set(data)
	}

	renderView(albums){
		if( this.props.view == 'list' ){

			var columns = [
				{ 
					label: 'Name', 
					name: 'name', 
					width: 40
				},
				{ 
					label: 'Artists', 
					name: 'artists', 
					width: 30
				},
				{ 
					label: 'Tracks', 
					name: 'tracks_total', 
					width: 15
				}
			]

			return (
				<section className="list-wrapper">
					<List 
						columns={columns} 
						rows={albums} 
						className="library-local-album-list"
						link_prefix={global.baseURL+"album/"} />
				</section>
			)
		}else{
			return (
				<section className="grid-wrapper">
					<AlbumGrid albums={albums} />
				</section>
			)
		}
	}

	render(){
		var albums = []
		if (this.props.albums && this.props.local_albums){
			for (var i = 0; i < this.props.local_albums.length; i++){
				var uri = this.props.local_albums[i]
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
				value: 'tracks_total',
				label: 'Tracks'
			}
		]

		var options = (
			<span>
				<DropdownField icon="sort" name="Sort" value={this.props.sort} options={sort_options} reverse={this.props.sort_reverse} handleChange={val => {this.setSort(val); this.props.uiActions.hideContextMenu() }} />
				<DropdownField icon="eye" name="View" value={this.props.view} options={view_options} handleChange={val => {this.props.uiActions.set({ library_local_albums_view: val }); this.props.uiActions.hideContextMenu() }} />
			</span>
		)

		if (albums.length <= 0 && helpers.isLoading(this.props.load_queue,['mopidy_lookup','mopidy_browse'])){
			return (
				<div className="view library-local-view">
					<Header icon="music" title="Local albums" options={options} uiActions={this.props.uiActions} />
					<div className="body-loader">
						<div className="loader"></div>
					</div>
				</div>
			)
		}

		return (
			<div className="view library-local-view">
				<Header icon="music" title="Local albums" options={options} uiActions={this.props.uiActions} />
				{this.renderView(albums)}
				<LazyLoadListener enabled={(this.moreURIsToLoad().length > 0)} loadMore={ () => this.loadMore() }/>
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
		mopidy_connected: state.mopidy.connected,
		albums: state.ui.albums,
		local_albums: state.ui.local_albums,
		view: state.ui.library_local_albums_view,
		sort: (state.ui.library_local_albums_sort ? state.ui.library_local_albums_sort : 'name'),
		sort_reverse: (state.ui.library_local_albums_sort_reverse ? true : false)
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryLocalAlbums)
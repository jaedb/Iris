
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import LazyLoadListener from '../../components/LazyLoadListener'
import Header from '../../components/Header'
import ArtistGrid from '../../components/ArtistGrid'
import List from '../../components/List'
import DropdownField from '../../components/DropdownField'

import * as helpers from '../../helpers'
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryArtists extends React.Component{

	constructor(props) {
		super(props);
	}

	handleContextMenu(e,item){
		var data = {
			e: e,
			context: 'artist',
			uris: [item.uri],
			items: [item]
		}
		this.props.uiActions.showContextMenu(data)
	}

	componentDidMount(){
		if (!this.props.library_artists_started){
			this.props.spotifyActions.getLibraryArtists();
		}
	}

	loadMore(){
		this.props.spotifyActions.getURL( this.props.library_artists_more, 'SPOTIFY_LIBRARY_ARTISTS_LOADED' );
	}

	setSort(value){
		var reverse = false
		if( this.props.sort == value ) reverse = !this.props.sort_reverse

		var data = {
			library_artists_sort_reverse: reverse,
			library_artists_sort: value
		}
		this.props.uiActions.set(data)
	}

	renderView(artists){
		if( this.props.view == 'list' ){
			var columns = [
				{
					label: 'Name',
					name: 'name'
				},
				{
					label: 'Followers',
					name: 'followers.total'
				},
				{
					label: 'Popularity',
					name: 'popularity'
				}
			]
			return (
				<section className="list-wrapper">
					<List 
						handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
						rows={artists} 
						columns={columns} 
						className="artist-list"
						link_prefix={global.baseURL+"artist/"} />
				</section>
			)
		}else{
			return (
				<section className="grid-wrapper">
					<ArtistGrid 
						handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
						artists={artists} />
				</section>				
			)
		}
	}

	render(){
		if (helpers.isLoading(this.props.load_queue,['spotify_me/following?type=artist'])){
			return (
				<div className="view library-albums-view">
					<Header icon="cd" title="My albums" />
					<div className="body-loader">
						<div className="loader"></div>
					</div>
				</div>
			)
		}

		var artists = []
		if (this.props.library_artists && this.props.artists){
			for (var i = 0; i < this.props.library_artists.length; i++){
				var uri = this.props.library_artists[i]
				if (this.props.artists.hasOwnProperty(uri)){
					artists.push(this.props.artists[uri])
				}
			}

			if( this.props.sort ){
				artists = helpers.sortItems(artists, this.props.sort, this.props.sort_reverse)
			}
		}

		var view_options = [
			{
				label: 'Thumbnails',
				value: 'thumbnails'
			},
			{
				label: 'List',
				value: 'list'
			}
		]

		var sort_options = [
			{
				label: 'Name',
				value: 'name'
			},
			{
				label: 'Followers',
				value: 'followers.total'
			},
			{
				label: 'Popularity',
				value: 'popularity'
			}
		]

		var options = (
			<span>
				<DropdownField icon="sort" name="Sort" value={ this.props.sort } options={sort_options} reverse={this.props.sort_reverse} handleChange={value => {this.setSort(value); this.props.uiActions.hideContextMenu() }} />
				<DropdownField icon="eye" name="View" value={ this.props.view } options={view_options} handleChange={value => {this.props.uiActions.set({ library_artists_view: value }); this.props.uiActions.hideContextMenu()}} />
			</span>
		)

		return (
			<div className="view library-artists-view">
				<Header icon="mic" title="My artists" options={options} uiActions={this.props.uiActions} />				
				{ this.renderView(artists) }
				<LazyLoadListener enabled={this.props.library_artists_more} loadMore={ () => this.loadMore() }/>
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
		artists: state.ui.artists,
		sort: (state.ui.library_artists_sort ? state.ui.library_artists_sort : 'name'),
		sort_reverse: (state.ui.library_artists_sort_reverse ? true : false),
		library_artists: state.ui.library_artists,
		library_artists_started: state.ui.library_artists_started,
		library_artists_more: state.ui.library_artists_more,
		library_artists_started: state.ui.library_artists_started,
		view: state.ui.library_artists_view
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryArtists)

import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'
import FontAwesome from 'react-fontawesome'

import PlaylistGrid from '../../components/PlaylistGrid'
import List from '../../components/List'
import DropdownField from '../../components/DropdownField'
import Header from '../../components/Header'

import * as helpers from '../../helpers'
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryPlaylists extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		if (!this.props.local_albums){
			if (this.props.spotify_connected){
				this.props.spotifyActions.getAllLibraryPlaylists()
			}
			if (this.props.mopidy_connected){
				this.props.mopidyActions.getLibraryPlaylists()
			}
		}
	}

	componentWillReceiveProps(newProps){
		if (!this.props.spotify_connected && newProps.spotify_connected){
			this.props.spotifyActions.getAllLibraryPlaylists()
		}

		if (!this.props.mopidy_connected && newProps.mopidy_connected){
			this.props.mopidyActions.getLibraryPlaylists()
		}
	}

	handleContextMenu(e,item){
		var data = {
			e: e,
			context: 'playlist',
			uris: [item.uri],
			items: [item]
		}
		this.props.uiActions.showContextMenu(data)
	}

	setSort(value){
		var reverse = false
		if( this.props.sort == value ) reverse = !this.props.sort_reverse

		var data = {
			library_playlists_sort_reverse: reverse,
			library_playlists_sort: value
		}
		this.props.uiActions.set(data)
	}

	renderView(){
		if (helpers.isLoading(this.props.load_queue,['spotify_me/playlists'])){
			return (
				<div className="body-loader loading">
					<div className="loader"></div>
				</div>
			)
		}

		if (!this.props.library_playlists || !this.props.playlists ){
			return null
		}

		var playlists = []
		for (var i = 0; i < this.props.library_playlists.length; i++){
			var uri = this.props.library_playlists[i]
			var owner_id = helpers.getFromUri('playlistowner',uri)

			if (this.props.playlists.hasOwnProperty(uri)){

				switch (this.props.filter){

					case 'only_mine':
						if (this.props.me_id && owner_id == this.props.me_id){
							playlists.push(this.props.playlists[uri])
						}
						break

					case 'only_others':
						if (!this.props.me_id || owner_id != this.props.me_id){
							playlists.push(this.props.playlists[uri])
						}
						break

					default:
						playlists.push(this.props.playlists[uri])
						break
				}
			}
		}

		if( this.props.sort ){
			playlists = helpers.sortItems(playlists, this.props.sort, this.props.sort_reverse)
		}

		if( this.props.view == 'list' ){
			if (this.props.slim_mode){
				var columns = [
					{
						label: 'Name',
						name: 'name'
					},
					{
						label: 'Tracks',
						name: 'tracks_total'
					}
				]
			} else {
				var columns = [
					{
						label: 'Name',
						name: 'name'
					},
					{
						label: 'Owner',
						name: 'owner'
					},
					{
						label: 'Tracks',
						name: 'tracks_total'
					},
					{
						label: 'Editable',
						name: 'can_edit'
					},
					{
						label: 'Source',
						name: 'source'
					}
				]
			}

			return (
				<section className="content-wrapper">
					<List
						handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
						rows={playlists}
						columns={columns}
						className="playlist-list"
						link_prefix={global.baseURL+"playlist/"} />
				</section>
			)
		}else{
			return (
				<section className="content-wrapper">
					<PlaylistGrid
						handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
						playlists={playlists} />
				</section>				
			)
		}
	}

	render(){

		var filter_options = [
			{
				value: 'all',
				label: 'All'
			},
			{
				value: 'only_mine',
				label: 'Owned by me'
			},
			{
				value: 'only_others',
				label: 'I\'m following'
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
				value: 'can_edit',
				label: 'Editable'
			},
			{
				value: 'owner.id',
				label: 'Owner'
			},
			{
				value: 'tracks.total',
				label: 'Tracks'
			},
			{
				value: 'source',
				label: 'Source'
			}
		]

		var options = (
			<span>
				<DropdownField icon="filter" name="Filter" value={this.props.filter} options={filter_options} handleChange={val => {this.props.uiActions.set({ library_playlists_filter: val}); this.props.uiActions.hideContextMenu() }} />
				<DropdownField icon="sort" name="Sort" value={this.props.sort} options={sort_options} reverse={this.props.sort_reverse} handleChange={val => {this.setSort(val); this.props.uiActions.hideContextMenu() }} />
				<DropdownField icon="eye" name="View" value={this.props.view} options={view_options} handleChange={val => {this.props.uiActions.set({ library_playlists_view: val}); this.props.uiActions.hideContextMenu() }} />
				<button className="no-hover" onClick={ () => this.props.uiActions.openModal('create_playlist', {} ) }>
					<FontAwesome name="plus" />&nbsp;
					New
				</button>
			</span>
		)

		return (
			<div className="view library-playlists-view">
				<Header icon="playlist" title="My playlists" options={options} uiActions={this.props.uiActions} />
				{ this.renderView() }
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
		mopidy_connected: state.mopidy.connected,
		spotify_connected: state.spotify.connected,
		slim_mode: state.ui.slim_mode,
		load_queue: state.ui.load_queue,
		me_id: (state.spotify.me ? state.spotify.me.id : (state.ui.config && state.ui.config.spotify_username ? state.ui.config.spotify_username : false)),
		view: state.ui.library_playlists_view,
		filter: (state.ui.library_playlists_filter ? state.ui.library_playlists_filter : 'all'),
		sort: (state.ui.library_playlists_sort ? state.ui.library_playlists_sort : 'name'),
		sort_reverse: (state.ui.library_playlists_sort_reverse ? true : false),
		library_playlists: state.core.library_playlists,
		playlists: state.core.playlists
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryPlaylists)
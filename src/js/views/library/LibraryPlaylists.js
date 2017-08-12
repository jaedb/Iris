
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
import * as coreActions from '../../services/core/actions'
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryPlaylists extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		if (!this.props.mopidy_library_playlists && this.props.mopidy_connected && (this.props.filter == 'all' || this.props.filter == 'local')){
			this.props.mopidyActions.getLibraryPlaylists()
		}

		if (!this.props.spotify_library_playlists && this.props.spotify_connected && (this.props.filter == 'all' || this.props.filter == 'spotify')){
			this.props.spotifyActions.getLibraryPlaylists()
		}
	}

	componentWillReceiveProps(newProps){
		if (newProps.mopidy_connected && (newProps.filter == 'all' || newProps.filter == 'local')){

			// We've just connected
			if (!this.props.mopidy_connected){
				this.props.mopidyActions.getLibraryPlaylists()
			}		

			// Filter changed, but we haven't got this provider's library yet
			if (this.props.filter != 'all' && this.props.filter != 'local' && !newProps.mopidy_library_playlists){
				this.props.mopidyActions.getLibraryPlaylists()
			}			
		}

		if (newProps.spotify_connected && (newProps.filter == 'all' || newProps.filter == 'spotify')){

			// We've just connected
			if (!this.props.spotify_connected){
				this.props.spotifyActions.getLibraryPlaylists()
			}		

			// Filter changed, but we haven't got this provider's library yet
			if (this.props.filter != 'all' && this.props.filter != 'spotify' && !newProps.spotify_library_playlists){
				this.props.spotifyActions.getLibraryPlaylists()
			}			
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
		var playlists = []

		// Spotify library items
		if (this.props.spotify_library_playlists && (this.props.filter == 'all' || this.props.filter == 'spotify')){
			for (var i = 0; i < this.props.spotify_library_playlists.length; i++){
				var uri = this.props.spotify_library_playlists[i]
				if (this.props.playlists.hasOwnProperty(uri)){
					playlists.push(this.props.playlists[uri])
				}
			}
		}

		// Mopidy library items
		if (this.props.mopidy_library_playlists && (this.props.filter == 'all' || this.props.filter == 'local')){
			for (var i = 0; i < this.props.mopidy_library_playlists.length; i++){
				var uri = this.props.mopidy_library_playlists[i]
				if (this.props.playlists.hasOwnProperty(uri)){
					playlists.push(this.props.playlists[uri])
				}
			}
		}

		playlists = helpers.sortItems(playlists, this.props.sort, this.props.sort_reverse)
		playlists = helpers.removeDuplicates(playlists)

		if (this.props.view == 'list'){
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
		mopidy_library_playlists: state.mopidy.library_playlists,
		spotify_library_playlists: state.spotify.library_playlists,
		slim_mode: state.ui.slim_mode,
		load_queue: state.ui.load_queue,
		me_id: (state.spotify.me ? state.spotify.me.id : (state.ui.config && state.ui.config.spotify_username ? state.ui.config.spotify_username : false)),
		view: state.ui.library_playlists_view,
		filter: (state.ui.library_playlists_filter ? state.ui.library_playlists_filter : 'all'),
		sort: (state.ui.library_playlists_sort ? state.ui.library_playlists_sort : 'name'),
		sort_reverse: (state.ui.library_playlists_sort_reverse ? true : false),
		playlists: state.core.playlists
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

export default connect(mapStateToProps, mapDispatchToProps)(LibraryPlaylists)
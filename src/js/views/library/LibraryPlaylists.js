
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
				<div className="body-loader">
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
			if (this.props.playlists.hasOwnProperty(uri)){
				playlists.push(this.props.playlists[uri])
			}
		}

		if( this.props.sort ){
			playlists = helpers.sortItems(playlists, this.props.sort, this.props.sort_reverse)
		}

		if( this.props.view == 'list' ){
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
					label: 'Source',
					name: 'source'
				},
				{
					label: 'Tracks',
					name: 'tracks_total'
				},
				{
					label: 'Editable',
					name: 'can_edit'
				}
			]
			return (
				<section className="list-wrapper">
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
				<section className="grid-wrapper">
					<PlaylistGrid
						handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
						playlists={playlists} />
				</section>				
			)
		}
	}

	render(){

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
				<DropdownField icon="sort" name="Sort" value={this.props.sort} options={sort_options} reverse={this.props.sort_reverse} handleChange={val => {this.setSort(val); this.props.uiActions.hideContextMenu() }} />
				<DropdownField icon="eye" name="View" value={this.props.view} options={view_options} handleChange={val => {this.props.uiActions.set({ library_playlists_view: val}); this.props.uiActions.hideContextMenu() }} />
				<button onClick={ () => this.props.uiActions.openModal('create_playlist', {} ) }>
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
		load_queue: state.ui.load_queue,
		view: state.ui.library_playlists_view,
		sort: (state.ui.library_playlists_sort ? state.ui.library_playlists_sort : 'name'),
		sort_reverse: (state.ui.library_playlists_sort_reverse ? true : false),
		library_playlists: state.ui.library_playlists,
		playlists: state.ui.playlists
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

import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import LazyLoadListener from '../../components/LazyLoadListener'
import Header from '../../components/Header'
import ArtistGrid from '../../components/ArtistGrid'
import List from '../../components/List'
import DropdownField from '../../components/DropdownField'
import FilterField from '../../components/FilterField'

import * as helpers from '../../helpers'
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryArtists extends React.Component{

	constructor(props) {
		super(props)

		this.state = {
			filter: ''
		}
	}

	componentDidMount(){
		if (!this.props.mopidy_library_artists && this.props.mopidy_connected && (this.props.source == 'all' || this.props.source == 'local')){
			this.props.mopidyActions.getLibraryArtists()
		}

		if (!this.props.spotify_library_artists && this.props.spotify_connected && (this.props.source == 'all' || this.props.source == 'spotify')){
			this.props.spotifyActions.getLibraryArtists()
		}
	}

	componentWillReceiveProps(newProps){
		if (newProps.mopidy_connected && (newProps.source == 'all' || newProps.source == 'local')){

			// We've just connected
			if (!this.props.mopidy_connected){
				this.props.mopidyActions.getLibraryArtists()
			}		

			// Filter changed, but we haven't got this provider's library yet
			if (this.props.source != 'all' && this.props.source != 'local' && !newProps.mopidy_library_artists){
				this.props.mopidyActions.getLibraryArtists()
			}			
		}

		if (newProps.spotify_connected && (newProps.source == 'all' || newProps.source == 'spotify')){

			// We've just connected
			if (!this.props.spotify_connected){
				this.props.spotifyActions.getLibraryArtists()
			}		

			// Filter changed, but we haven't got this provider's library yet
			if (this.props.source != 'all' && this.props.source != 'spotify' && !newProps.spotify_library_artists){
				this.props.spotifyActions.getLibraryArtists()
			}			
		}
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
				<section className="content-wrapper">
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
				<section className="content-wrapper">
					<ArtistGrid 
						handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
						artists={artists} />
				</section>				
			)
		}
	}

	render(){
		var artists = []

		// Mopidy library items
		if (this.props.mopidy_library_artists && (this.props.source == 'all' || this.props.source == 'local')){
			for (var i = 0; i < this.props.mopidy_library_artists.length; i++){

				// Construct item placeholder. This is used as Mopidy needs to 
				// lookup ref objects to get the full object which can take some time
				var uri = this.props.mopidy_library_artists[i]
				var source = helpers.uriSource(uri)
				var artist = {
					uri: uri,
					source: source
				}

				if (this.props.artists.hasOwnProperty(uri)){
					artist = this.props.artists[uri]
				}

				artists.push(artist)
			}
		}

		// Spotify library items
		if (this.props.spotify_library_artists && (this.props.source == 'all' || this.props.source == 'spotify')){
			for (var i = 0; i < this.props.spotify_library_artists.length; i++){
				var uri = this.props.spotify_library_artists[i]
				if (this.props.artists.hasOwnProperty(uri)){
					artists.push(this.props.artists[uri])
				}
			}
		}

		artists = helpers.sortItems(artists, this.props.sort, this.props.sort_reverse)

		if (this.state.filter !== ''){
			artists = helpers.applyFilter('name', this.state.filter, artists)
		}

		var source_options = [
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
				<FilterField handleChange={value => this.setState({filter: value})} />
				<DropdownField icon="sort" name="Sort" value={ this.props.sort } options={sort_options} reverse={this.props.sort_reverse} handleChange={value => {this.setSort(value); this.props.uiActions.hideContextMenu() }} />
				<DropdownField icon="eye" name="View" value={ this.props.view } options={view_options} handleChange={value => {this.props.uiActions.set({ library_artists_view: value }); this.props.uiActions.hideContextMenu()}} />
				<DropdownField icon="database" name="Source" value={this.props.source} options={source_options} handleChange={val => {this.props.uiActions.set({ library_artists_source: val}); this.props.uiActions.hideContextMenu() }} />
			</span>
		)

		return (
			<div className="view library-artists-view">
				<Header icon="mic" title="My artists" options={options} uiActions={this.props.uiActions} />				
				{ this.renderView(artists) }
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
		mopidy_library_artists: state.mopidy.library_artists,
		spotify_library_artists: state.spotify.library_artists,
		artists: state.core.artists,
		source: (state.ui.library_artists_source ? state.ui.library_artists_source : 'all'),
		sort: (state.ui.library_artists_sort ? state.ui.library_artists_sort : 'name'),
		sort_reverse: (state.ui.library_artists_sort_reverse ? true : false),
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
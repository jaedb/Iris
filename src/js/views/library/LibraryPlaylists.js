
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import Link from '../../components/Link';

import PlaylistGrid from '../../components/PlaylistGrid'
import List from '../../components/List'
import DropdownField from '../../components/Fields/DropdownField'
import Header from '../../components/Header'
import FilterField from '../../components/Fields/FilterField'
import LazyLoadListener from '../../components/LazyLoadListener'
import Icon from '../../components/Icon'

import * as helpers from '../../helpers'
import * as coreActions from '../../services/core/actions'
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryPlaylists extends React.Component{

	constructor(props){
		super(props)

		this.state = {
			filter: '',
			limit: 50,
			per_page: 50
		}
	}

	componentDidMount(){
		this.props.uiActions.setWindowTitle("Playlists");

		if (!this.props.mopidy_library_playlists && this.props.mopidy_connected && (this.props.source == 'all' || this.props.source == 'local')){
			this.props.mopidyActions.getLibraryPlaylists()
		}

		if (this.props.spotify_enabled && this.props.spotify_library_playlists_status !== 'finished' && (this.props.source == 'all' || this.props.source == 'spotify')){
			this.props.spotifyActions.getLibraryPlaylists()
		}
	}

	componentWillReceiveProps(newProps){

		if (newProps.mopidy_connected && (newProps.source == 'all' || newProps.source == 'local')){

			// We've just connected
			if (!this.props.mopidy_connected){
				this.props.mopidyActions.getLibraryPlaylists();
			}		

			// Filter changed, but we haven't got this provider's library yet
			if (this.props.source != 'all' && this.props.source != 'local' && !newProps.mopidy_library_playlists){
				this.props.mopidyActions.getLibraryPlaylists()
			}			
		}

		if (newProps.spotify_enabled && (newProps.source == 'all' || newProps.source == 'spotify')){

			// Filter changed, but we haven't got this provider's library yet
			if (newProps.spotify_library_playlists_status != 'finished' && newProps.spotify_library_playlists_status != 'started'){
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
		if (this.props.sort == value ) reverse = !this.props.sort_reverse

		var data = {
			library_playlists_sort_reverse: reverse,
			library_playlists_sort: value
		}
		this.props.uiActions.set(data)
	}

	renderView(){
		var playlists = [];

		// Spotify library items
		if (this.props.spotify_library_playlists && (this.props.source == 'all' || this.props.source == 'spotify')){
			for (var i = 0; i < this.props.spotify_library_playlists.length; i++){
				var uri = this.props.spotify_library_playlists[i]
				if (this.props.playlists.hasOwnProperty(uri)){
					playlists.push(this.props.playlists[uri])
				}
			}
		}

		// Mopidy library items
		if (this.props.mopidy_library_playlists && (this.props.source == 'all' || this.props.source == 'local')){
			for (var i = 0; i < this.props.mopidy_library_playlists.length; i++){
				var uri = this.props.mopidy_library_playlists[i]
				if (this.props.playlists.hasOwnProperty(uri)){
					playlists.push(this.props.playlists[uri])
				}
			}
		}

		if (this.props.sort){
			playlists = helpers.sortItems(playlists, this.props.sort, this.props.sort_reverse);
		}
		playlists = helpers.removeDuplicates(playlists);

		if (this.state.filter !== ''){
			playlists = helpers.applyFilter('name', this.state.filter, playlists);
		}

		// Apply our lazy-load-rendering
		var total_playlists = playlists.length;
		playlists = playlists.slice(0, this.state.limit);

		if (this.props.view == 'list'){
			return (
				<section className="content-wrapper">
					<List
						handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
						rows={playlists}
						thumbnail={true}
						details={['owner','tracks_total']}
						right_column={['source']}
						className="playlists"
						link_prefix={global.baseURL+"playlist/"} />
					<LazyLoadListener 
						loadKey={this.state.limit}
						loading={this.state.limit < total_playlists} 
						loadMore={() => this.setState({limit: this.state.limit + this.state.per_page})}
					/>
				</section>
			)
		} else {
			return (
				<section className="content-wrapper">
					<PlaylistGrid
						handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
						playlists={playlists} />
					<LazyLoadListener 
						loadKey={this.state.limit}
						loading={this.state.limit < total_playlists}
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

		var view_options = [
			{
				value: 'thumbnails',
				label: 'Thumbnails'
			},
			{
				value: 'list',
				label: 'List'
			}
		];

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
		];

		var options = (
			<span>
				<FilterField 
					initialValue={this.state.filter}
					handleChange={value => this.setState({filter: value})}
				/>
				<DropdownField 
					icon="sort"
					name="Sort"
					value={this.props.sort}
					options={sort_options}
					selected_icon={this.props.sort ? (this.props.sort_reverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null} 
					handleChange={value => {this.setSort(value); this.props.uiActions.hideContextMenu() }}
				/>
				<DropdownField
					icon="visibility"
					name="View"
					value={this.props.view}
					options={view_options}
					handleChange={value => {this.props.uiActions.set({ library_playlists_view: value}); this.props.uiActions.hideContextMenu() }}
				/>
				<DropdownField
					icon="cloud"
					name="Source"
					value={this.props.source}
					options={source_options}
					handleChange={value => {this.props.uiActions.set({ library_playlists_source: value}); this.props.uiActions.hideContextMenu() }}
				/>
				<Link className="button no-hover" to={global.baseURL+'playlist/create'}>
					<Icon name="add_box" />New
				</Link>
			</span>
		)

		return (
			<div className="view library-playlists-view">
				<Header options={options} uiActions={this.props.uiActions}>				
					<Icon name="queue_music" type="material" />
					My playlists
				</Header>
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
		slim_mode: state.ui.slim_mode,
		mopidy_connected: state.mopidy.connected,
		mopidy_uri_schemes: state.mopidy.uri_schemes,
		mopidy_library_playlists: state.mopidy.library_playlists,
		mopidy_library_playlists_status: (state.ui.processes.MOPIDY_LIBRARY_PLAYLISTS_PROCESSOR !== undefined ? state.ui.processes.MOPIDY_LIBRARY_PLAYLISTS_PROCESSOR.status : null),
		spotify_enabled: (state.mopidy.uri_schemes && state.mopidy.uri_schemes.includes('spotify:')),
		spotify_library_playlists: state.spotify.library_playlists,
		spotify_library_playlists_status: (state.ui.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR !== undefined ? state.ui.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR.status : null),
		load_queue: state.ui.load_queue,
		me_id: (state.spotify.me ? state.spotify.me.id : false),
		view: state.ui.library_playlists_view,
		source: (state.ui.library_playlists_source ? state.ui.library_playlists_source : 'all'),
		sort: (state.ui.library_playlists_sort ? state.ui.library_playlists_sort : null),
		sort_reverse: (state.ui.library_playlists_sort_reverse ? state.ui.library_playlists_sort_reverse : false),
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
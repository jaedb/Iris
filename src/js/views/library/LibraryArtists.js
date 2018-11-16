
import React, { PropTypes } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Link from '../../components/Link';

import Header from '../../components/Header';
import ArtistGrid from '../../components/ArtistGrid';
import List from '../../components/List';
import DropdownField from '../../components/Fields/DropdownField';
import FilterField from '../../components/Fields/FilterField';
import LazyLoadListener from '../../components/LazyLoadListener';
import Icon from '../../components/Icon';

import * as helpers from '../../helpers';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import * as googleActions from '../../services/google/actions';

class LibraryArtists extends React.Component{

	constructor(props){
		super(props)

		this.state = {
			filter: '',
			limit: 50,
			per_page: 50
		}
	}

	componentDidMount(){
		this.props.uiActions.setWindowTitle("Artists");

		if (!this.props.mopidy_library_artists && this.props.mopidy_connected && (this.props.source == 'all' || this.props.source == 'local')){
			this.props.mopidyActions.getLibraryArtists();
		}

		if (this.props.google_enabled && !this.props.google_library_artists && this.props.mopidy_connected && (this.props.source == 'all' || this.props.source == 'google')){
			this.props.googleActions.getLibraryArtists();
		}

		if (this.props.spotify_enabled && this.props.spotify_library_artists_status != 'finished' && (this.props.source == 'all' || this.props.source == 'spotify')){
			this.props.spotifyActions.getLibraryArtists();
		}
	}

	componentWillReceiveProps(newProps){
		if (newProps.mopidy_connected && (newProps.source == 'all' || newProps.source == 'local')){

			// We've just connected
			if (!this.props.mopidy_connected){
				this.props.mopidyActions.getLibraryArtists();
			}		

			// Filter changed, but we haven't got this provider's library yet
			if (this.props.source != 'all' && this.props.source != 'local' && !newProps.mopidy_library_artists){
				this.props.mopidyActions.getLibraryArtists();
			}			
		}

		if (newProps.mopidy_connected && newProps.google_enabled && (newProps.source == 'all' || newProps.source == 'google')){

			// We've just been enabled (or detected as such)
			if (!this.props.google_enabled){
				this.props.googleActions.getLibraryArtists();
			}		

			// Filter changed, but we haven't got this provider's library yet
			if (this.props.source != 'all' && this.props.source != 'google' && !newProps.google_library_artists){
				this.props.googleActions.getLibraryArtists();
			}
		}

		if (newProps.spotify_enabled && (newProps.source == 'all' || newProps.source == 'spotify')){

			// Filter changed, but we haven't got this provider's library yet
			if (newProps.spotify_library_artists_status != 'finished' && newProps.spotify_library_artists_status != 'started'){
				this.props.spotifyActions.getLibraryArtists();
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

	loadMore(){
		console.log('Load more');
		this.setState({limit: this.state.limit + this.state.per_page});
	}

	setSort(value){
		var reverse = false
		if (this.props.sort == value ) reverse = !this.props.sort_reverse

		var data = {
			library_artists_sort_reverse: reverse,
			library_artists_sort: value
		}
		this.props.uiActions.set(data);
	}

	renderView(){
		var artists = [];

		// Mopidy library items
		if (this.props.mopidy_library_artists && (this.props.source == 'all' || this.props.source == 'local')){
			for (uri of this.props.mopidy_library_artists){

				// Construct item placeholder. This is used as Mopidy needs to 
				// lookup ref objects to get the full object which can take some time
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

		// Google library items
		if (this.props.google_library_artists && (this.props.source == 'all' || this.props.source == 'google')){
			for (uri of this.props.google_library_artists){

				// Construct item placeholder. This is used as Mopidy needs to 
				// lookup ref objects to get the full object which can take some time
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

		if (this.props.sort){
			artists = helpers.sortItems(artists, this.props.sort, this.props.sort_reverse);
		}

		if (this.state.filter !== ''){
			artists = helpers.applyFilter('name', this.state.filter, artists)
		}

		// Apply our lazy-load-rendering
		var total_artists = artists.length;
		artists = artists.slice(0, this.state.limit);

		if (this.props.view == 'list'){
			return (
				<section className="content-wrapper">
					<List 
						handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
						rows={artists} 
						thumbnail={true}
						details={['followers']}
						middle_column={['source']}
						className="artists"
						link_prefix={global.baseURL+"artist/"} />
					<LazyLoadListener 
						loadKey={this.state.limit}
						showLoader={this.state.limit < total_artists}
						loadMore={() => this.loadMore()}
					/>
				</section>
			)
		} else {
			return (
				<section className="content-wrapper">
					<ArtistGrid 
						handleContextMenu={(e,item) => this.handleContextMenu(e,item)}
						artists={artists} />
					<LazyLoadListener 
						loadKey={this.state.limit}
						showLoader={this.state.limit < total_artists} 
						loadMore={() => this.loadMore()}
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

		if (this.props.google_enabled){
			source_options.push({
				value: 'google',
				label: 'Google'
			});
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
		];

		var sort_options = [
			{
				label: 'Default',
				value: null
			},
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
		];

		var options = (
			<span>
				<FilterField 
					initialValue={this.state.filter}
					handleChange={value => this.setState({filter: value, limit: this.state.per_page})} 
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
					handleChange={value => {this.props.uiActions.set({ library_artists_view: value }); this.props.uiActions.hideContextMenu()}}
				/>
				<DropdownField
					icon="cloud"
					name="Source"
					value={this.props.source}
					options={source_options}
					handleChange={value => {this.props.uiActions.set({ library_artists_source: value}); this.props.uiActions.hideContextMenu() }}
				/>
			</span>
		)

		return (
			<div className="view library-artists-view">
				<Header options={options} uiActions={this.props.uiActions}>				
					<Icon name="recent_actors" type="material" />
					My artists
				</Header>	
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
		mopidy_library_artists: state.mopidy.library_artists,
		google_enabled: state.google.enabled,
		google_library_artists: state.google.library_artists,
		spotify_enabled: state.spotify.enabled,
		spotify_library_artists: state.spotify.library_artists,
		spotify_library_artists_status: (state.ui.processes.SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR !== undefined ? state.ui.processes.SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR.status : null),
		artists: state.core.artists,
		source: (state.ui.library_artists_source ? state.ui.library_artists_source : 'all'),
		sort: (state.ui.library_artists_sort ? state.ui.library_artists_sort : null),
		sort_reverse: (state.ui.library_artists_sort_reverse ? state.ui.library_artists_sort_reverse : false),
		view: state.ui.library_artists_view
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		googleActions: bindActionCreators(googleActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryArtists)
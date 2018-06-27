
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link, hashHistory } from 'react-router'
import { bindActionCreators } from 'redux'

import Header from '../components/Header'
import Icon from '../components/Icon'
import DropdownField from '../components/Fields/DropdownField'
import TrackList from '../components/TrackList'
import ArtistGrid from '../components/ArtistGrid'
import AlbumGrid from '../components/AlbumGrid'
import PlaylistGrid from '../components/PlaylistGrid'
import LazyLoadListener from '../components/LazyLoadListener'
import SearchForm from '../components/Fields/SearchForm'
import URILink from '../components/URILink'

import * as helpers from '../helpers'
import * as coreActions from '../services/core/actions'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Search extends React.Component{

	constructor(props){
		super(props);

		this.state = {
			type: 'all',
			term: ''
		}
	}

	componentDidMount(){
		this.props.uiActions.setWindowTitle("Search");

		// Auto-focus on the input field
		$(document).find('.search-form input').focus();

		// Listen for a query baked-in to the URL
		// This would be the case when we've clicked from a link elsewhere
		this.digestUri();
	}

	componentWillUnmount(){
		this.props.mopidyActions.clearSearchResults();
		this.props.spotifyActions.clearSearchResults();
	}

	componentWillReceiveProps(nextProps){

		// Query changed
		if (nextProps.params.query !== this.props.params.query){
			this.digestUri(nextProps);
		}

		// Services came online
		if (!this.props.mopidy_connected && nextProps.mopidy_connected && nextProps.uri_schemes_search_enabled){
			this.search(this.state.type, this.state.term, 'mopidy');

			if (nextProps.uri_schemes_search_enabled.includes('spotify:')){
				this.search(this.state.type, this.state.term, 'spotify');
			}
		}
	}

	// Digest the URI query property
	// Triggered when the URL changes
	digestUri(props = this.props){
		if (props.params && props.params.query && props.params.query !== ''){
			var type = helpers.getFromUri("searchtype", props.params.query);
			var term = helpers.getFromUri("searchterm", props.params.query);

			if (type){
				this.setState({type: type});
			}

			if (term){
				this.setState({term: term});
			}

			if (type && term){
				this.search(type, term);
			}
		}
	}

	search(type = this.state.type, term = this.state.term, provider){

		this.props.mopidyActions.clearSearchResults();
		this.props.spotifyActions.clearSearchResults();

		if (type && term){

			if (provider == 'mopidy' || (this.props.mopidy_connected && this.props.uri_schemes_search_enabled)){
				this.props.mopidyActions.getSearchResults(type, term)
			}

			if (provider == 'spotify' || (this.props.mopidy_connected && this.props.uri_schemes_search_enabled && this.props.uri_schemes_search_enabled.includes('spotify:'))){
				this.props.spotifyActions.getSearchResults(type, term)
			}
		}
	}

	loadMore(type){
		alert('load more: '+type)
		//this.props.spotifyActions.getURL(this.props['spotify_'+type+'_more'], 'SPOTIFY_SEARCH_RESULTS_LOADED_MORE_'+type.toUpperCase());
	}

	setSort(value){
		var reverse = false
		if (this.props.sort == value ) reverse = !this.props.sort_reverse

		var data = {
			search_results_sort_reverse: reverse,
			search_results_sort: value
		}
		this.props.uiActions.set(data)
	}

	renderResults(){
		var spotify_search_enabled = (this.props.search_settings && this.props.search_settings.spotify);

		var sort = this.props.sort;
		var sort_reverse = this.props.sort_reverse;
		var sort_map = null;

		switch (this.props.sort){
			case 'uri':
				sort_map = this.props.uri_schemes_priority;
				break;

			case 'followers.total':
				sort_reverse = !sort_reverse;
				break;
		}

		var artists = [];
		if (this.props.mopidy_search_results.artists){
			artists = [...artists, ...helpers.getIndexedRecords(this.props.artists,this.props.mopidy_search_results.artists)];
		}
		if (this.props.spotify_search_results.artists){
			artists = [...artists, ...helpers.getIndexedRecords(this.props.artists,this.props.spotify_search_results.artists)];
		}
		artists = helpers.sortItems(artists, sort, sort_reverse, sort_map);

		var albums = [];
		if (this.props.mopidy_search_results.albums){
			albums = [...albums, ...helpers.getIndexedRecords(this.props.albums,this.props.mopidy_search_results.albums)];
		}
		if (this.props.spotify_search_results.albums){
			albums = [...albums, ...helpers.getIndexedRecords(this.props.albums,this.props.spotify_search_results.albums)]
		}
		albums = helpers.sortItems(albums, sort, sort_reverse, sort_map);

		var playlists = []
		if (this.props.mopidy_search_results.playlists){
			playlists = [...playlists, ...helpers.getIndexedRecords(this.props.playlists,this.props.mopidy_search_results.playlists)];
		}
		if (this.props.spotify_search_results.playlists){
			playlists = [...playlists, ...helpers.getIndexedRecords(this.props.playlists,this.props.spotify_search_results.playlists)];
		}
		playlists = helpers.sortItems(playlists, sort, sort_reverse, sort_map);

		var tracks = [];
		if (this.props.mopidy_search_results.tracks){
			tracks = [...tracks, ...this.props.mopidy_search_results.tracks];
		}
		if (this.props.spotify_search_results.tracks){
			tracks = [...tracks, ...this.props.spotify_search_results.tracks];
		}

		tracks = helpers.sortItems(tracks, (sort == 'followers.total' ? 'popularity' : sort), sort_reverse, sort_map);
		
		switch (this.state.type){

			case 'artist':
				return (
					<div>
						<h4>
							<URILink unencoded type="search" uri={"search:all:"+this.state.term}>
								Search
							</URILink>
							&nbsp; <Icon type="fontawesome" name="angle-right" />&nbsp;
							Artists
						</h4>
						<section className="grid-wrapper">
							<ArtistGrid artists={artists} show_source_icon />
							<LazyLoadListener enabled={this.props['artists_more'] && spotify_search_enabled} loadMore={ () => this.loadMore('artists') }/>
						</section>
					</div>
				)
				break;

			case 'album':
				return (
					<div>
						<h4>
							<URILink unencoded type="search" uri={"search:all:"+this.state.term}>
								Search
							</URILink>
							&nbsp; <Icon type="fontawesome" name="angle-right" />&nbsp;
							Albums
						</h4>
						<section className="grid-wrapper">
							<AlbumGrid albums={albums} show_source_icon />
							<LazyLoadListener enabled={this.props['albums_more'] && spotify_search_enabled} loadMore={ () => this.loadMore('albums') }/>
						</section>
					</div>
				)
				break;

			case 'playlist':
				return (
					<div>
						<h4>
							<URILink unencoded type="search" uri={"search:all:"+this.state.term}>
								Search
							</URILink>
							&nbsp; <Icon type="fontawesome" name="angle-right" />&nbsp;
							Playlists
						</h4>
						<section className="grid-wrapper">
							<PlaylistGrid playlists={playlists} show_source_icon />
							<LazyLoadListener enabled={this.props['playlists_more'] && spotify_search_enabled} loadMore={ () => this.loadMore('playlists') }/>
						</section>
					</div>
				)
				break;

			case 'track':
				return (
					<div>
						<h4>
							<URILink unencoded type="search" uri={"search:all:"+this.state.term}>
								Search
							</URILink>
							&nbsp; <Icon type="fontawesome" name="angle-right" />&nbsp;
							Tracks
						</h4>
						<section className="list-wrapper">
							<TrackList tracks={tracks} uri={'iris:'+this.props.params.query} show_source_icon />
							<LazyLoadListener enabled={this.props['tracks_more'] && spotify_search_enabled} loadMore={ () => this.loadMore('tracks') }/>
						</section>
					</div>
				)
				break;

			case 'all':
			default:
			
				if (artists.length > 0){
					var artists_section = (					
						<section>
							<div className="inner">								
								<URILink unencoded type="search" uri={"search:artist:"+this.state.term}>
									<h4>Artists</h4>
								</URILink>
								<ArtistGrid show_source_icon artists={artists.slice(0,5)} />
								{artists.length > 4 ? <URILink unencoded type="search" uri={"search:artist:"+this.state.term} className="button grey">
									All artists ({artists.length})
								</URILink> : null}
							</div>
						</section>
					)
				} else {
					var artists_section = null;
				}

				if (albums.length > 0){
					var albums_section = (					
						<section>
							<div className="inner">						
								<URILink unencoded type="search" uri={"search:album:"+this.state.term}>
									<h4>Albums</h4>
								</URILink>
								<AlbumGrid show_source_icon albums={albums.slice(0,5)} />
								{albums.length > 4 ? <URILink unencoded type="search" uri={"search:album:"+this.state.term} className="button grey">
									All albums ({albums.length})
								</URILink> : null}
							</div>
						</section>
					)
				} else {
					var albums_section = null;
				}
			
				if (playlists.length > 0){
					var playlists_section = (					
						<section>
							<div className="inner">						
								<URILink unencoded type="search" uri={"search:playlist:"+this.state.term}>
									<h4>Playlists</h4>
								</URILink>
								<PlaylistGrid show_source_icon playlists={playlists.slice(0,5)} />
								{playlists.length > 4 ? <URILink unencoded type="search" uri={"search:playlist:"+this.state.term} className="button grey">
									All playlists ({playlists.length})
								</URILink> : null}
							</div>
						</section>
					)
				} else {
					var playlists_section = null;
				}

				if (tracks.length > 0){
					var tracks_section = (
						<section className="list-wrapper">
							<TrackList tracks={tracks} uri={'iris:'+this.props.params.query} show_source_icon />
							<LazyLoadListener loading={this.props['tracks_more'] && spotify_search_enabled} loadMore={ () => this.loadMore('tracks') }/>
						</section>
					)
				} else {
					var tracks_section = null;
				}

				return (
					<div>
						<div className="search-result-sections cf">
							{artists_section}
							{albums_section}
							{playlists_section}
						</div>
						{tracks_section}
					</div>
				)
		}
	}

	render(){
		var type_options = [
			{
				value: 'all',
				label: 'All'
			},
			{
				value: 'artist',
				label: 'Artist'
			},
			{
				value: 'album',
				label: 'Album'
			},
			{
				value: 'playlist',
				label: 'Playlist'
			},
			{
				value: 'track',
				label: 'Track'
			}
		];

		var sort_options = [
			{
				value: 'followers.total',
				label: 'Popularity'
			},
			{
				value: 'name',
				label: 'Name'
			},
			{
				value: 'artists.name',
				label: 'Artist'
			},
			{
				value: 'duration',
				label: 'Duration'
			},
			{
				value: 'uri',
				label: 'Source'
			}
		];

		var provider_options = [];
		for (var i = 0; i < this.props.uri_schemes.length; i++){
			provider_options.push({
				value: this.props.uri_schemes[i],
				label: helpers.titleCase(this.props.uri_schemes[i].replace(':','').replace('+',' '))
			});
		}

		var options = (
			<span>
				<DropdownField 
					icon="category" 
					name="Type" 
					value={this.state.type} 
					options={type_options} 
					handleChange={value => {this.setState({type: value}); this.search(value, this.state.term)}}
				/>
				<DropdownField 
					icon="sort" 
					name="Sort" 
					value={this.props.sort} 
					options={sort_options} 
					selected_icon={this.props.sort_reverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down'} 
					handleChange={value => {this.setSort(value); this.props.uiActions.hideContextMenu()}}
				/>
				<DropdownField 
					icon="cloud" 
					name="Sources"
					value={this.props.uri_schemes_search_enabled}
					options={provider_options} 
					handleChange={value => {this.props.uiActions.set({uri_schemes_search_enabled: value}); this.props.uiActions.hideContextMenu()}}
				/>
			</span>
		)

		return (
			<div className="view search-view">			
				<Header options={options} uiActions={this.props.uiActions}>
					<Icon name="search" type="material" />
				</Header>

				<SearchForm 
					term={this.state.term}
					onBlur={term => this.setState({term: term})}
					onSubmit={term => this.search(this.state.type, term)}
				/>

				<div className="content-wrapper">
					{ this.renderResults() }
				</div>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		mopidy_connected: state.mopidy.connected,
		albums: (state.core.albums ? state.core.albums : []),
		artists: (state.core.artists ? state.core.artists : []),
		playlists: (state.core.playlists ? state.core.playlists : []),
		tracks: (state.core.tracks ? state.core.tracks : []),
		uri_schemes_search_enabled: (state.ui.uri_schemes_search_enabled ? state.ui.uri_schemes_search_enabled : []),
		uri_schemes_priority: (state.ui.uri_schemes_priority ? state.ui.uri_schemes_priority : []),
		uri_schemes: (state.mopidy.uri_schemes ? state.mopidy.uri_schemes : []),
		mopidy_search_results: (state.mopidy.search_results ? state.mopidy.search_results : {}),
		spotify_search_results: (state.spotify.search_results ? state.spotify.search_results : {}),
		sort: (state.ui.search_results_sort ? state.ui.search_results_sort : 'followers.total'),
		sort_reverse: (state.ui.search_results_sort_reverse ? true : false)
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

export default connect(mapStateToProps, mapDispatchToProps)(Search)
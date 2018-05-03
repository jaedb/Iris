
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link, hashHistory } from 'react-router'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'
import ReactGA from 'react-ga'

import Header from '../components/Header'
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
	}

	componentDidMount(){
		var context = "all";
		var term = null;
		if (this.props.params && this.props.params.query && this.props.params.query !== ''){
			context = helpers.getFromUri("searchcontext",this.props.params.query);
			term = helpers.getFromUri("searchterm",this.props.params.query);
		}

		// Auto-focus on the input field
		$(document).find('.search-form input').focus();

		if (context && term){
			if (this.props.mopidy_connected && this.props.search_uri_schemes){
				this.props.mopidyActions.getSearchResults(context, term)
			}

			if (this.props.search_uri_schemes && this.props.search_uri_schemes.includes('spotify:')){
				this.props.spotifyActions.getSearchResults(context, term)
			}
		}
	}

	componentWillReceiveProps(newProps){
		if (this.props.params && this.props.params.query && this.props.params.query !== ''){
			var old_context = helpers.getFromUri("searchcontext",this.props.params.query);
			var old_term = helpers.getFromUri("searchterm",this.props.params.query);
		} else {
			var old_context = "all";
			var old_term = null;
		}

		if (newProps.params && newProps.params.query && newProps.params.query !== ''){
			var context = helpers.getFromUri("searchcontext",newProps.params.query);
			var term = helpers.getFromUri("searchterm",newProps.params.query);
		} else {
			var context = "all";
			var term = null;
		}

		// Search changed 
		if (term && context && (term !== old_term || context !== old_context)){
			
			this.props.mopidyActions.clearSearchResults();
			this.props.spotifyActions.clearSearchResults();

			if (this.props.mopidy_connected && this.props.search_uri_schemes){
				this.props.mopidyActions.getSearchResults(context, term)
			}

			if (this.props.mopidy_connected && this.props.search_uri_schemes && this.props.search_uri_schemes.includes('spotify:')){
				this.props.spotifyActions.getSearchResults(context, term)
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

		var context = helpers.getFromUri("searchcontext",this.props.params.query);
		var term = helpers.getFromUri("searchterm",this.props.params.query);
		if (!context){
			context = "all";
		}

		var spotify_search_enabled = (this.props.search_settings && this.props.search_settings.spotify);

		if (this.props.sort == 'uri'){
			var sort_map = this.props.search_uri_schemes;
		} else {
			var sort_map = null;
		}

		var artists = [];
		if (this.props.mopidy_search_results.artists){
			artists = [...artists, ...helpers.getIndexedRecords(this.props.artists,this.props.mopidy_search_results.artists)];
		}
		if (this.props.spotify_search_results.artists){
			artists = [...artists, ...helpers.getIndexedRecords(this.props.artists,this.props.spotify_search_results.artists)];
		}
		artists = helpers.sortItems(artists, this.props.sort, this.props.sort_reverse, sort_map);

		var albums = [];
		if (this.props.mopidy_search_results.albums){
			albums = [...albums, ...helpers.getIndexedRecords(this.props.albums,this.props.mopidy_search_results.albums)];
		}
		if (this.props.spotify_search_results.albums){
			albums = [...albums, ...helpers.getIndexedRecords(this.props.albums,this.props.spotify_search_results.albums)]
		}		
		albums = helpers.sortItems(albums, this.props.sort, this.props.sort_reverse, sort_map);

		var playlists = []
		if (this.props.mopidy_search_results.playlists){
			playlists = [...playlists, ...helpers.getIndexedRecords(this.props.playlists,this.props.mopidy_search_results.playlists)];
		}
		if (this.props.spotify_search_results.playlists){
			playlists = [...playlists, ...helpers.getIndexedRecords(this.props.playlists,this.props.spotify_search_results.playlists)];
		}
		playlists = helpers.sortItems(playlists, this.props.sort, this.props.sort_reverse, sort_map);

		var tracks = [];
		if (this.props.mopidy_search_results.tracks){
			tracks = [...tracks, ...this.props.mopidy_search_results.tracks];
		}
		if (this.props.spotify_search_results.tracks){
			tracks = [...tracks, ...this.props.spotify_search_results.tracks];
		}
		tracks = helpers.sortItems(tracks, this.props.sort, this.props.sort_reverse, sort_map);
		
		switch (context){

			case 'artist':
				return (
					<div>
						<h4>
							<URILink unencoded type="search" uri={"search:all:"+term}>
								Search
							</URILink>
							&nbsp; <FontAwesome name="angle-right" />&nbsp;
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
							<URILink unencoded type="search" uri={"search:all:"+term}>
								Search
							</URILink>
							&nbsp; <FontAwesome name="angle-right" />&nbsp;
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
							<URILink unencoded type="search" uri={"search:all:"+term}>
								Search
							</URILink>
							&nbsp; <FontAwesome name="angle-right" />&nbsp;
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
							<URILink unencoded type="search" uri={"search:all:"+term}>
								Search
							</URILink>
							&nbsp; <FontAwesome name="angle-right" />&nbsp;
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
								<URILink unencoded type="search" uri={"search:artist:"+term}>
									<h4>Artists</h4>
								</URILink>
								<ArtistGrid show_source_icon artists={artists.slice(0,5)} />
								{artists.length > 4 ? <URILink unencoded type="search" uri={"search:artist:"+term} className="button grey">
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
								<URILink unencoded type="search" uri={"search:album:"+term}>
									<h4>Albums</h4>
								</URILink>
								<AlbumGrid show_source_icon albums={albums.slice(0,5)} />
								{albums.length > 4 ? <URILink unencoded type="search" uri={"search:album:"+term} className="button grey">
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
								<URILink unencoded type="search" uri={"search:playlist:"+term}>
									<h4>Playlists</h4>
								</URILink>
								<PlaylistGrid show_source_icon playlists={playlists.slice(0,5)} />
								{playlists.length > 4 ? <URILink unencoded type="search" uri={"search:playlist:"+term} className="button grey">
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
		var sort_options = [
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
			}
		];

		var provider_options = [{
			value: 'all',
			label: 'All'
		}];
		for (var i = 0; i < this.props.uri_schemes.length; i++){
			provider_options.push({
				value: this.props.uri_schemes[i],
				label: helpers.titleCase(this.props.uri_schemes[i].replace(':','').replace('+',' '))
			});
		}

		var options = (
			<span>
				<DropdownField 
					icon="sort" 
					name="Sort" 
					value={this.props.sort} 
					options={sort_options} 
					reverse={this.props.sort_reverse} 
					handleChange={value => {this.setSort(value); this.props.uiActions.hideContextMenu()}}
				/>
				<DropdownField 
					icon="database" 
					name="Source"
					value={this.props.search_uri_schemes}
					options={provider_options} 
					reverse={this.props.sort_reverse} 
					handleChange={value => {this.props.uiActions.set({search_uri_schemes: value}); this.props.uiActions.hideContextMenu()}}
				/>
			</span>
		)

		return (
			<div className="view search-view">			
				<Header
					icon="search" 
					options={options} 
					uiActions={this.props.uiActions}
				/>

				<SearchForm 
					query={(this.props.params.query ? this.props.params.query : '')}
					view={(this.props.params.view ? this.props.params.view : 'all')}
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
		search_uri_schemes: (state.ui.search_uri_schemes ? state.ui.search_uri_schemes : []),
		uri_schemes: (state.mopidy.uri_schemes ? state.mopidy.uri_schemes : []),
		mopidy_search_results: (state.mopidy.search_results ? state.mopidy.search_results : {}),
		spotify_search_results: (state.spotify.search_results ? state.spotify.search_results : {}),
		sort: (state.ui.search_results_sort ? state.ui.search_results_sort : 'name'),
		sort_reverse: (state.ui.search_results_sort_reverse ? true : false),
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
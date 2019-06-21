
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { Switch, Route } from 'react-router-dom';

import Link from '../components/Link';
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

	componentWillReceiveProps(nextProps){

		// Query changed
		if (nextProps.term !== this.props.term || nextProps.type !== this.props.type){
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

	handleSubmit(term){
		this.setState(
			{term: term}, () =>
			{
				// Unchanged term, so this is a forced re-search
				// Often the other search parameters have changed instead, but we can't
				// push a URL change when the term hasn't changed
				if (this.props.term == term){
					this.search();
				} else {
					this.props.history.push(`/search/${this.state.type}/${term}`);
				}
			}
		);
	}

	// Digest the URI query property
	// Triggered when the URL changes
	digestUri(props = this.props){
		if (props.type && props.term){
			this.setState({
				type: props.type,
				term: props.term
			});

			this.search(props.type, props.term);
		} else if (!props.term || props.term == ''){
			this.props.spotifyActions.clearSearchResults();
			this.props.mopidyActions.clearSearchResults();
		}
	}

	search(type = this.state.type, term = this.state.term, provider){

		this.props.uiActions.setWindowTitle("Search: "+term);

		if (type && term){

			if (provider == 'mopidy' || (this.props.mopidy_connected && this.props.uri_schemes_search_enabled)){
				if (this.props.mopidy_search_results.query === undefined || this.props.mopidy_search_results.query != term){
					this.props.mopidyActions.clearSearchResults();
					this.props.mopidyActions.getSearchResults(type, term);
				}
			}

			if (provider == 'spotify' || (this.props.mopidy_connected && this.props.uri_schemes_search_enabled && this.props.uri_schemes_search_enabled.includes('spotify:'))){
				if (this.props.spotify_search_results.query === undefined || this.props.spotify_search_results.query != term){
					this.props.spotifyActions.clearSearchResults();
					this.props.spotifyActions.getSearchResults(type, term);
				}
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

	renderArtists(artists, spotify_search_enabled){	
		return (
			<div>
				<h4>
					<URILink type="search" uri={"search:all:"+this.state.term}>
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
		);
	}

	renderAlbums(albums, spotify_search_enabled){	
		return (
			<div>
				<h4>
					<URILink type="search" uri={"search:all:"+this.state.term}>
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
		);
	}

	renderPlaylists(playlists, spotify_search_enabled){	
		return (
			<div>
				<h4>
					<URILink type="search" uri={"search:all:"+this.state.term}>
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
		);
	}

	renderTracks(tracks, spotify_search_enabled){	
		return (
			<div>
				<h4>
					<URILink type="search" uri={"search:all:"+this.state.term}>
						Search
					</URILink>
					&nbsp; <Icon type="fontawesome" name="angle-right" />&nbsp;
					Tracks
				</h4>
				<section className="list-wrapper">
					<TrackList tracks={tracks} uri={'iris:search:'+this.state.type+':'+this.state.term} show_source_icon />
					<LazyLoadListener enabled={this.props['tracks_more'] && spotify_search_enabled} loadMore={ () => this.loadMore('tracks') }/>
				</section>
			</div>
		);
	}

	renderAll(artists, albums, playlists, tracks, spotify_search_enabled){		
		if (artists.length > 0){
			var artists_section = (					
				<section>
					<div className="inner">								
						<URILink type="search" uri={"search:artist:"+this.state.term}>
							<h4>Artists</h4>
						</URILink>
						<ArtistGrid mini show_source_icon artists={artists.slice(0,6)} />
						{artists.length >= 6 ? <URILink type="search" uri={"search:artist:"+this.state.term} className="button button--default">
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
						<URILink type="search" uri={"search:album:"+this.state.term}>
							<h4>Albums</h4>
						</URILink>
						<AlbumGrid mini show_source_icon albums={albums.slice(0,6)} />
						{albums.length >= 6 ? <URILink type="search" uri={"search:album:"+this.state.term} className="button button--default">
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
						<URILink type="search" uri={"search:playlist:"+this.state.term}>
							<h4>Playlists</h4>
						</URILink>
						<PlaylistGrid mini show_source_icon playlists={playlists.slice(0,6)} />
						{playlists.length >= 6 ? <URILink type="search" uri={"search:playlist:"+this.state.term} className="button button--default">
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
					<TrackList tracks={tracks} uri={'iris:search:'+this.state.type+':'+this.state.term} show_source_icon />
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

	render(){
		var sort_options = [
			{
				value: 'followers',
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
		var spotify_search_enabled = (this.props.search_settings && this.props.search_settings.spotify);

		var sort = this.props.sort;
		var sort_reverse = this.props.sort_reverse;
		var sort_map = null;

		switch (this.props.sort){
			case 'uri':
				sort_map = this.props.uri_schemes_priority;
				break;

			// Followers (aka popularlity works in reverse-numerical order)
			// Ie "more popular" is a bigger number
			case 'followers':
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

		tracks = helpers.sortItems(tracks, (sort == 'followers' ? 'popularity' : sort), sort_reverse, sort_map);

		var options = (
			<span>
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
					history={this.props.history}
					term={this.state.term}
					onSubmit={term => this.handleSubmit(term)}
				/>

				<div className="content-wrapper">
					<Switch>

						<Route path="/search/artist/:term">
							{this.renderArtists(artists, spotify_search_enabled)}
						</Route>

						<Route path="/search/album/:term">
							{this.renderAlbums(albums, spotify_search_enabled)}
						</Route>

						<Route path="/search/playlist/:term">
							{this.renderPlaylists(playlists, spotify_search_enabled)}
						</Route>

						<Route path="/search/track/:term">
							{this.renderTracks(tracks, spotify_search_enabled)}
						</Route>

						<Route path="/search">
							{this.renderAll(artists, albums, playlists, tracks, spotify_search_enabled)}
						</Route>

					</Switch>
				</div>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		type: ownProps.match.params.type,
		term: ownProps.match.params.term,
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
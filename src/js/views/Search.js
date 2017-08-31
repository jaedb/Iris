
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link, hashHistory } from 'react-router'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'
import ReactGA from 'react-ga'

import Header from '../components/Header'
import DropdownField from '../components/DropdownField'
import TrackList from '../components/TrackList'
import ArtistGrid from '../components/ArtistGrid'
import AlbumGrid from '../components/AlbumGrid'
import PlaylistGrid from '../components/PlaylistGrid'
import LazyLoadListener from '../components/LazyLoadListener'
import SearchForm from '../components/SearchForm'

import * as helpers from '../helpers'
import * as coreActions from '../services/core/actions'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Search extends React.Component{

	constructor(props) {
		super(props)
	}

	componentDidMount(){

		// Make sure we have search parameters to start with
		if (this.props.params && this.props.params.type && this.props.params.query){
			this.props.coreActions.startSearch(this.props.params.type, this.props.params.query)
		}

		// Auto-focus on the input field
		$(document).find('.search-form input').focus();
	}

	componentWillReceiveProps(newProps){
		
		// Make sure we have some search parameters
		if (newProps.params && newProps.params.type && newProps.params.query){

			if (this.props.params.query != newProps.params.query || this.props.params.type != newProps.params.type){
				this.props.coreActions.startSearch(newProps.params.type, newProps.params.query)
			}

			// mopidy comes online
			if (!this.props.mopidy_connected && newProps.mopidy_connected){
				this.props.coreActions.startSearch(newProps.params.type, newProps.params.query, true)
			}
		}
	}

	loadMore(type){
		this.props.spotifyActions.getURL( this.props[type+'_more'], 'SPOTIFY_SEARCH_RESULTS_LOADED_MORE_'+type.toUpperCase());
	}

	renderResults(){
		var spotify_search_enabled = (this.props.search_settings && this.props.search_settings.spotify)

		var artists = []
		if (this.props.spotify_search_results.artists){
			artists = [...artists, ...helpers.getIndexedRecords(this.props.artists,this.props.spotify_search_results.artists)]
		}
		if (this.props.mopidy_search_results.artists){
			artists = [...artists, ...helpers.getIndexedRecords(this.props.artists,this.props.mopidy_search_results.artists)]
		}

		var albums = []
		if (this.props.spotify_search_results.albums){
			albums = [...albums, ...helpers.getIndexedRecords(this.props.albums,this.props.spotify_search_results.albums)]
		}
		if (this.props.mopidy_search_results.albums){
			albums = [...albums, ...helpers.getIndexedRecords(this.props.albums,this.props.mopidy_search_results.albums)]
		}

		var playlists = []
		if (this.props.spotify_search_results.playlists){
			playlists = [...playlists, ...helpers.getIndexedRecords(this.props.playlists,this.props.spotify_search_results.playlists)]
		}
		if (this.props.mopidy_search_results.playlists){
			playlists = [...playlists, ...helpers.getIndexedRecords(this.props.playlists,this.props.mopidy_search_results.playlists)]
		}

		var tracks = []
		if (this.props.spotify_search_results.tracks){
			tracks = [...tracks, ...this.props.spotify_search_results.tracks]
		}
		if (this.props.mopidy_search_results.tracks){
			tracks = [...tracks, ...this.props.mopidy_search_results.tracks]
		}

		switch (this.props.params.type){

			case 'artists':
				return (
					<div>
						<section className="grid-wrapper">
							<ArtistGrid artists={artists} show_source_icon />
							<LazyLoadListener enabled={this.props['artists_more'] && spotify_search_enabled} loadMore={ () => this.loadMore('artists') }/>
						</section>
					</div>
				)
				break

			case 'albums':
				return (
					<div>
						<section className="grid-wrapper">
							<AlbumGrid albums={albums} show_source_icon />
							<LazyLoadListener enabled={this.props['albums_more'] && spotify_search_enabled} loadMore={ () => this.loadMore('albums') }/>
						</section>
					</div>
				)
				break

			case 'playlists':
				return (
					<div>
						<section className="grid-wrapper">
							<PlaylistGrid playlists={playlists} show_source_icon />
							<LazyLoadListener enabled={this.props['playlists_more'] && spotify_search_enabled} loadMore={ () => this.loadMore('playlists') }/>
						</section>
					</div>
				)
				break

			case 'tracks':
				return (
					<div>
						<section className="list-wrapper">
							<TrackList tracks={tracks} uri={'iris:search:'+this.props.params.type+':'+this.props.params.query} show_source_icon />
							<LazyLoadListener enabled={this.props['tracks_more'] && spotify_search_enabled} loadMore={ () => this.loadMore('tracks') }/>
						</section>
					</div>
				)
				break

			default:
			
				if (artists.length > 0){
					var artists_section = (					
						<section>
							<div className="inner">
								<h4><Link to={global.baseURL+'search/iris:search:artists:'+this.props.params.query}>Artists</Link></h4>
								<ArtistGrid show_source_icon artists={artists.slice(0,5)} />
							</div>
						</section>
					)
				} else {
					var artists_section = null
				}

				if (albums.length > 0){
					var albums_section = (					
						<section>
							<div className="inner">
								<h4><Link to={global.baseURL+'search/iris:search:albums:'+this.props.params.query}>Albums</Link></h4>
								<AlbumGrid show_source_icon albums={albums.slice(0,5)} />
							</div>
						</section>
					)
				} else {
					var albums_section = null
				}
			
				if (playlists.length > 0){
					var playlists_section = (					
						<section>
							<div className="inner">
								<h4><Link to={global.baseURL+'search/iris:search:playlists:'+this.props.params.query}>Playlists</Link></h4>
								<PlaylistGrid show_source_icon playlists={playlists.slice(0,5)} />
							</div>
						</section>
					)
				} else {
					var playlists_section = null
				}

				return (
					<div>
						<div className="search-result-sections cf">
							{artists_section}
							{albums_section}
							{playlists_section}
						</div>

						<section className="list-wrapper">
							<h4><Link to={global.baseURL+'search/'+this.props.params.query+'/tracks'}>Tracks</Link></h4>
							<TrackList tracks={tracks} uri={'iris:search:'+this.props.params.type+':'+this.props.params.query} show_source_icon />
							<LazyLoadListener loading={this.props['tracks_more'] && spotify_search_enabled} loadMore={ () => this.loadMore('tracks') }/>
						</section>

					</div>
				)
		}
	}

	handleTypeChange(val){
		this.props.uiActions.hideContextMenu()
		hashHistory.push(global.baseURL+'search/iris:search:'+val+':'+this.props.params.query)
	}

	render(){
		var type_options = [
			{
				value: 'all',
				label: 'All'
			},
			{
				value: 'artists',
				label: 'Artists'
			},
			{
				value: 'albums',
				label: 'Albums'
			},
			{
				value: 'playlists',
				label: 'Playlists'
			},
			{
				value: 'tracks',
				label: 'Tracks'
			}
		]

		var options = (
			<span>
				<DropdownField icon="eye" name="Type" value={this.props.params.type} options={type_options} handleChange={val => this.handleTypeChange(val)} />
				<button className="no-hover" onClick={e => this.props.uiActions.openModal('search_settings', {type: this.props.params.type, query: this.props.params.query})}>
					<FontAwesome name="wrench" />&nbsp;
					Sources
				</button>
			</span>
		)

		return (
			<div className="view search-view">			
				<Header icon="search" options={options} uiActions={this.props.uiActions} />
				<SearchForm query={this.props.params.query} />
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
		spotify_connected: state.spotify.connected,
		albums: (state.core.albums ? state.core.albums : []),
		artists: (state.core.artists ? state.core.artists : []),
		playlists: (state.core.playlists ? state.core.playlists : []),
		tracks: (state.core.tracks ? state.core.tracks : []),
		search_settings: (state.ui.search_settings ? state.ui.search_settings : {}),
		mopidy_search_results: (state.mopidy.search_results ? state.mopidy.search_results : {}),
		spotify_search_results: (state.spotify.search_results ? state.spotify.search_results : {})
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
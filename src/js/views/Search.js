
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

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Search extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.props.uiActions.startSearch(this.props.params.type, this.props.params.query)
	}

	componentWillReceiveProps(newProps){

		if (this.props.params.query != newProps.params.query || this.props.params.type != newProps.params.type){
			this.props.uiActions.startSearch(newProps.params.type, newProps.params.query)
		}

		// mopidy comes online
		if (!this.props.mopidy_connected && newProps.mopidy_connected){
			this.props.uiActions.startSearch(newProps.params.type, newProps.params.query, true)
		}
	}

	loadMore(type){
		this.props.spotifyActions.getURL( this.props[type+'_more'], 'SPOTIFY_SEARCH_RESULTS_LOADED_MORE_'+type.toUpperCase());
	}

	renderResults(){
		var spotify_search_enabled = (this.props.search_settings && this.props.search_settings.spotify)

		var artists = []
		if (this.props.artists_uris){
			for (var i = 0; i < this.props.artists_uris.length; i++){
				var uri = this.props.artists_uris[i]
				if (this.props.artists.hasOwnProperty(uri)){
					artists.push(this.props.artists[uri])
				}
			}
		}

		var albums = []
		if (this.props.albums_uris){
			for (var i = 0; i < this.props.albums_uris.length; i++){
				var uri = this.props.albums_uris[i]
				if (this.props.albums.hasOwnProperty(uri)){
					albums.push(this.props.albums[uri])
				}
			}
		}

		var playlists = []
		if (this.props.playlists_uris){
			for (var i = 0; i < this.props.playlists_uris.length; i++){
				var uri = this.props.playlists_uris[i]
				if (this.props.playlists.hasOwnProperty(uri)){
					playlists.push(this.props.playlists[uri])
				}
			}
		}

		var tracks = []
		if (this.props.tracks){
			for (var i = 0; i < this.props.tracks.length; i++){
				if (helpers.uriSource(this.props.tracks[i].uri)){
					tracks.push(this.props.tracks[i])
				}
			}
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
							<h4 className="left-padding"><Link to={global.baseURL+'search/'+this.props.params.query+'/tracks'}>Tracks</Link></h4>
							<TrackList tracks={tracks} uri={'iris:search:'+this.props.params.type+':'+this.props.params.query} show_source_icon />
							<LazyLoadListener enabled={this.props['tracks_more'] && spotify_search_enabled} loadMore={ () => this.loadMore('tracks') }/>
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
				<button onClick={e => this.props.uiActions.openModal('search_settings', {type: this.props.params.type, query: this.props.params.query})}>
					<FontAwesome name="wrench" />&nbsp;
					Sources
				</button>
			</span>
		)

		return (
			<div className="view search-view">			
				<Header icon="search" title="Search results" options={options} uiActions={this.props.uiActions} />
				{ this.renderResults() }
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		mopidy_connected: state.mopidy.connected,
		search_settings: (state.ui.search_settings ? state.ui.search_settings : null),
		tracks: (state.ui.search_results ? state.ui.search_results.tracks : []),
		tracks_more: (state.ui.search_results && state.ui.search_results.tracks_more ? state.ui.search_results.tracks_more : null),
		artists: (state.ui.artists ? state.ui.artists : []),
		artists_uris: (state.ui.search_results ? state.ui.search_results.artists_uris : []),
		artists_more: (state.ui.search_results ? state.ui.search_results.artists_more : null),
		albums: (state.ui.albums ? state.ui.albums : []),
		albums_uris: (state.ui.search_results ? state.ui.search_results.albums_uris : []),
		albums_more: (state.ui.search_results ? state.ui.search_results.albums_more : null),
		playlists: (state.ui.playlists ? state.ui.playlists : []),
		playlists_uris: (state.ui.search_results ? state.ui.search_results.playlists_uris : []),
		playlists_more: (state.ui.search_results ? state.ui.search_results.playlists_more : null)
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Search)
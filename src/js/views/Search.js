
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
		this.performSearch()
	}

	componentWillReceiveProps(newProps){

		// new query
		if( this.props.params.query != newProps.params.query ){
			this.performSearch( newProps )
		}

		// mopidy comes online
		if( !this.props.mopidy_connected && newProps.mopidy_connected ){
			this.props.mopidyActions.getSearchResults( newProps.params.query, newProps.uri_schemes )
		}
	}

	performSearch( props = this.props ){
		var source = props.params.source
		var type = props.params.type

		this.props.uiActions.startSearch(props.params.query)

		if (!source || source == 'all' || source == 'spotify'){
			this.props.spotifyActions.getSearchResults( props.params.query )
		}

		if( props.mopidy_connected ){
			this.props.mopidyActions.getSearchResults( props.params.query, props.uri_schemes )
		}
	}

	loadMore(type){
		this.props.spotifyActions.getURL( this.props[type+'_more'], 'SPOTIFY_SEARCH_RESULTS_LOADED_MORE_'+type.toUpperCase());
	}

	renderResults(){
		var type = (this.props.params.type ? this.props.params.type : 'all')
		var source = (this.props.params.source ? this.props.params.source : 'all')

		var artists = []
		if (this.props.artists_uris){
			for (var i = 0; i < this.props.artists_uris.length; i++){
				var uri = this.props.artists_uris[i]
				if (this.props.artists.hasOwnProperty(uri) && (source == 'all' || helpers.uriSource(uri) == source)){
					artists.push(this.props.artists[uri])
				}
			}
		}

		var albums = []
		if (this.props.albums_uris){
			for (var i = 0; i < this.props.albums_uris.length; i++){
				var uri = this.props.albums_uris[i]
				if (this.props.albums.hasOwnProperty(uri) && (source == 'all' || helpers.uriSource(uri) == source)){
					albums.push(this.props.albums[uri])
				}
			}
		}

		var playlists = []
		if (this.props.playlists_uris){
			for (var i = 0; i < this.props.playlists_uris.length; i++){
				var uri = this.props.playlists_uris[i]
				if (this.props.playlists.hasOwnProperty(uri) && (source == 'all' || helpers.uriSource(uri) == source)){
					playlists.push(this.props.playlists[uri])
				}
			}
		}

		var tracks = []
		if (this.props.tracks){
			if (source == 'all'){
				tracks = this.props.tracks
			} else {
				for (var i = 0; i < this.props.tracks.length; i++){
					if (helpers.uriSource(this.props.tracks[i].uri) == source){
						tracks.push(this.props.tracks[i])
					}
				}
			}
		}

		switch( type ){

			case 'artists':
				return (
					<div>
						<section className="grid-wrapper">
							<ArtistGrid artists={artists} />
							<LazyLoadListener enabled={this.props['artists_more'] && (source == 'spotify' || source == 'all')} loadMore={ () => this.loadMore('artists') }/>
						</section>
					</div>
				)
				break

			case 'albums':
				return (
					<div>
						<section className="grid-wrapper">
							<AlbumGrid albums={albums} />
							<LazyLoadListener enabled={this.props['albums_more'] && (source == 'spotify' || source == 'all')} loadMore={ () => this.loadMore('albums') }/>
						</section>
					</div>
				)
				break

			case 'playlists':
				return (
					<div>
						<section className="grid-wrapper">
							<PlaylistGrid playlists={playlists} />
							<LazyLoadListener enabled={this.props['playlists_more'] && (source == 'spotify' || source == 'all')} loadMore={ () => this.loadMore('playlists') }/>
						</section>
					</div>
				)
				break

			case 'tracks':
				return (
					<div>
						<section className="list-wrapper">
							<TrackList show_source_icon={true} tracks={ tracks } />
							<LazyLoadListener enabled={this.props['tracks_more'] && (source == 'spotify' || source == 'all')} loadMore={ () => this.loadMore('tracks') }/>
						</section>
					</div>
				)
				break

			default:
				return (
					<div>
						<div className="search-result-sections cf">
							<section>
								<div className="inner">
									<h4><Link to={global.baseURL+'search/'+this.props.params.query+'/artists'}>Artists</Link></h4>
									<ArtistGrid className="mini" artists={artists.slice(0,6)} />
								</div>
							</section>
							<section>
								<div className="inner">
									<h4><Link to={global.baseURL+'search/'+this.props.params.query+'/albums'}>Albums</Link></h4>
									<AlbumGrid className="mini" albums={albums.slice(0,6)} />
								</div>
							</section>
							<section>
								<div className="inner">
									<h4><Link to={global.baseURL+'search/'+this.props.params.query+'/playlists'}>Playlists</Link></h4>
									<PlaylistGrid className="mini" playlists={playlists.slice(0,6)} />
								</div>
							</section>
						</div>

						<section className="list-wrapper">
							<h4 className="left-padding"><Link to={global.baseURL+'search/'+this.props.params.query+'/tracks'}>Tracks</Link></h4>
							<TrackList show_source_icon={true} tracks={ tracks } />
							<LazyLoadListener enabled={this.props['tracks_more'] && (source == 'spotify' || source == 'all')} loadMore={ () => this.loadMore('tracks') }/>
						</section>

					</div>
				)
		}
	}

	handleTypeChange(val){
		this.props.uiActions.hideContextMenu()
		var source = (this.props.params.source ? this.props.params.source : 'all')
		hashHistory.push(global.baseURL+'search/'+this.props.params.query+'/'+source+'/'+val)
	}

	handleSourceChange(val){
		this.props.uiActions.hideContextMenu()
		var type = (this.props.params.type ? this.props.params.type : 'all')
		hashHistory.push(global.baseURL+'search/'+this.props.params.query+'/'+val+'/'+type)
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

		var source_options = [
			{
				value: 'all',
				label: 'All'
			},
			{
				value: 'spotify',
				label: 'Spotify'
			}
		]
		for (var i = 0; i < this.props.uri_schemes.length; i++){
			var scheme = this.props.uri_schemes[i].replace(':','')
			source_options.push({
				value: scheme,
				label: scheme
			})
		}

		var options = (
			<span>
				<DropdownField name="Filter" value={this.props.params.type} options={type_options} handleChange={val => this.handleTypeChange(val)} />
				<DropdownField name="Source" value={this.props.params.source} options={source_options} handleChange={val => this.handleSourceChange(val)} />
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
		uri_schemes: state.mopidy.uri_schemes,
		tracks: (state.ui.search_results ? state.ui.search_results.tracks : []),
		tracks_more: (state.ui.search_results && state.ui.search_results.tracks_more ? state.ui.search_results.tracks_more : null),
		artists: state.ui.artists,
		artists_uris: (state.ui.search_results ? state.ui.search_results.artists_uris : []),
		artists_more: (state.ui.search_results ? state.ui.search_results.artists_more : null),
		albums: state.ui.albums,
		albums_uris: (state.ui.search_results ? state.ui.search_results.albums_uris : []),
		albums_more: (state.ui.search_results ? state.ui.search_results.albums_more : null),
		playlists: state.ui.playlists,
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
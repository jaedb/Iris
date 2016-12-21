
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import Header from '../components/Header'
import TrackList from '../components/TrackList'
import ArtistGrid from '../components/ArtistGrid'
import AlbumGrid from '../components/AlbumGrid'
import PlaylistGrid from '../components/PlaylistGrid'
import LazyLoadListener from '../components/LazyLoadListener'

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
		this.props.uiActions.searchStarted()
		this.props.spotifyActions.getSearchResults( props.params.query )
		this.props.mopidyActions.getSearchResults( props.params.query, props.uri_schemes )
	}

	loadMore(type){
		if( !this.props.search_results[type] || 
			!this.props.search_results[type+'_more'] ){
			return
		}

		this.props.spotifyActions.getURL( 
			this.props.search_results[type+'_more'], 
			'SPOTIFY_SEARCH_RESULTS_LOADED_MORE_'+ type.toUpperCase()
		);
	}

	renderResults(){
		switch( this.props.params.type ){

			case 'artists':
				return (
					<div>
						<section className="grid-wrapper">
							<ArtistGrid artists={ this.props.search_results.artists } />
							<LazyLoadListener loadMore={ () => this.loadMore('artists') }/>
						</section>
					</div>
				)
				break

			case 'albums':
				return (
					<div>
						<section className="grid-wrapper">
							<AlbumGrid albums={ this.props.search_results.albums } />
							<LazyLoadListener loadMore={ () => this.loadMore('albums') }/>
						</section>
					</div>
				)
				break

			case 'playlists':
				return (
					<div>
						<section className="grid-wrapper">
							<PlaylistGrid playlists={ this.props.search_results.playlists } />
							<LazyLoadListener loadMore={ () => this.loadMore('playlists') }/>
						</section>
					</div>
				)
				break

			case 'tracks':
				return (
					<div>
						<section className="list-wrapper">
							<TrackList show_source_icon={true} tracks={ this.props.search_results.tracks } />
							<LazyLoadListener loadMore={ () => this.loadMore('tracks') }/>
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
									<ArtistGrid className="mini" artists={ this.props.search_results.artists.slice(0,6) } />
								</div>
							</section>
							<section>
								<div className="inner">
									<h4><Link to={global.baseURL+'search/'+this.props.params.query+'/albums'}>Albums</Link></h4>
									<AlbumGrid className="mini" albums={ this.props.search_results.albums.slice(0,6) } />
								</div>
							</section>
							<section>
								<div className="inner">
									<h4><Link to={global.baseURL+'search/'+this.props.params.query+'/playlists'}>Playlists</Link></h4>
									<PlaylistGrid className="mini" playlists={ this.props.search_results.playlists.slice(0,6) } />
								</div>
							</section>
						</div>

						<section className="list-wrapper">
							<h4 className="left-padding"><Link to={global.baseURL+'search/'+this.props.params.query+'/tracks'}>Tracks</Link></h4>
							<TrackList show_source_icon={true} tracks={ this.props.search_results.tracks } />
							<LazyLoadListener loadMore={ () => this.loadMore('tracks') }/>
						</section>

					</div>
				)
		}
	}

	render(){
		return (
			<div className="view search-view">			
				<Header icon="search" title="Search results" />
				{ this.renderResults() }
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		mopidy_connected: state.mopidy.connected,
		uri_schemes: state.mopidy.uri_schemes,
		search_results: state.ui.search_results
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
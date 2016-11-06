
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
		if( this.props.params.query != newProps.params.query ){
			this.performSearch( newProps )
		}
	}

	performSearch( props = this.props ){
		this.props.spotifyActions.getSearchResults( props.params.query )
		if( props.mopidy.connected ) this.props.mopidyActions.getSearchResults( props.params.query )
	}

	loadMore(type){
		if( !this.props.spotify['search_results_'+type] || 
			!this.props.spotify['search_results_'+type].next ){
			return
		}

		this.props.spotifyActions.getURL( 
			this.props.spotify['search_results_'+type].next, 
			'SPOTIFY_SEARCH_RESULTS_LOADED_MORE_'+ type.toUpperCase()
		);
	}

	compiledResults(type){
		var results = [];

		// merge our mopidy results in
		if( this.props.mopidy['search_results_'+type] ){
			results = [...results, ...this.props.mopidy['search_results_'+type]]
		}

		// merge our spotify results in
		if( this.props.spotify['search_results_'+type] && 
			this.props.spotify['search_results_'+type].items ){
			results = [...results, ...this.props.spotify['search_results_'+type].items]
		}

		return results;
	}

	renderResults(){
		switch( this.props.params.type ){

			case 'artists':
				return (
					<div>
						<section className="grid-wrapper">
							<ArtistGrid artists={ this.compiledResults('artists') } />
						</section>
						<LazyLoadListener loadMore={ () => this.loadMore('artists') }/>
					</div>
				)
				break

			case 'albums':
				return (
					<div>
						<section className="grid-wrapper">
							<AlbumGrid albums={ this.compiledResults('albums') } />
						</section>
						<LazyLoadListener loadMore={ () => this.loadMore('albums') }/>
					</div>
				)
				break

			case 'playlists':
				return (
					<div>
						<section className="grid-wrapper">
							<PlaylistGrid playlists={ this.compiledResults('playlists') } />
						</section>
						<LazyLoadListener loadMore={ () => this.loadMore('playlists') }/>
					</div>
				)
				break

			case 'tracks':
				return (
					<div>
						<section className="list-wrapper">
							<TrackList show_source_icon={true} tracks={ this.compiledResults('tracks') } />
						</section>
						<LazyLoadListener loadMore={ () => this.loadMore('tracks') }/>
					</div>
				)
				break

			default:
				return (
					<div>
						<div className="search-result-sections cf">
							<section>
								<div className="inner">
									<h4><Link to={'/search/'+this.props.params.query+'/artists'}>Artists</Link></h4>
									<ArtistGrid className="mini" artists={ this.compiledResults('artists').splice(0,6) } />
								</div>
							</section>
							<section>
								<div className="inner">
									<h4><Link to={'/search/'+this.props.params.query+'/albums'}>Albums</Link></h4>
									<AlbumGrid className="mini" albums={ this.compiledResults('albums').splice(0,6) } />
								</div>
							</section>
							<section>
								<div className="inner">
									<h4><Link to={'/search/'+this.props.params.query+'/playlists'}>Playlists</Link></h4>
									<PlaylistGrid className="mini" playlists={ this.compiledResults('playlists').splice(0,6) } />
								</div>
							</section>
						</div>

						<section className="list-wrapper">
							<h4 className="left-padding"><Link to={'/search/'+this.props.params.query+'/tracks'}>Tracks</Link></h4>
							<TrackList show_source_icon={true} tracks={ this.compiledResults('tracks') } />
						</section>

						<LazyLoadListener loadMore={ () => this.loadMore('tracks') }/>
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
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Search)
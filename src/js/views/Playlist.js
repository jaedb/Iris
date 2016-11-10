
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'
let helpers = require('../helpers.js')

import TrackList from '../components/TrackList'
import Thumbnail from '../components/Thumbnail'
import Dater from '../components/Dater'
import LazyLoadListener from '../components/LazyLoadListener'

import * as spotifyActions from '../services/spotify/actions'
import * as mopidyActions from '../services/mopidy/actions'

class Playlist extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.loadPlaylist();
	}

	componentWillReceiveProps( nextProps ){
		if( nextProps.params.uri != this.props.params.uri ){
			this.loadPlaylist( nextProps )
		}else if( !this.props.mopidy_connected && nextProps.mopidy_connected ){
			if( helpers.uriSource( this.props.params.uri ) == 'm3u' ){
				this.loadPlaylist( nextProps )
			}
		}
	}

	loadPlaylist( props = this.props ){
		var source = helpers.uriSource( props.params.uri );
		if( source == 'spotify' ){
			this.props.spotifyActions.getPlaylist( props.params.uri );
		}else if( source == 'm3u' && props.mopidy_connected ){
			this.props.mopidyActions.getPlaylist( props.params.uri );
		}
	}

	loadMore(){
		if( !this.props.playlist.tracks_more ) return
		this.props.spotifyActions.getURL( this.props.playlist.tracks_more, 'SPOTIFY_PLAYLIST_LOADED_MORE' );
	}

	render(){
		if( !this.props.playlist ) return null;
		var scheme = helpers.uriSource( this.props.params.uri );

		return (
			<div className="view playlist-view">
				<div className="intro">
					<Thumbnail size="large" images={ this.props.playlist.images } />
					<ul className="details">
						<li>{ this.props.playlist.tracks_total } tracks, <Dater type="total-time" data={this.props.playlist.tracks} /></li>
						{ this.props.playlist.last_modified ? <li>Updated <Dater type="ago" data={this.props.playlist.last_modified} /> ago</li> : null }
						{ scheme == 'spotify' ? <li><FontAwesome name="spotify" /> Spotify playlist</li> : null }
						{ scheme == 'm3u' ? <li><FontAwesome name="folder" /> Local playlist</li> : null }
					</ul>
				</div>
				<div className="main">

					<div className="title">
						<h1>{ this.props.playlist.name }</h1>
					</div>

					<section className="list-wrapper">
						<TrackList tracks={ this.props.playlist.tracks } />
						<LazyLoadListener loadMore={ () => this.loadMore() }/>
					</section>
					
				</div>
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
		playlist: state.ui.playlist,
		mopidy_connected: state.mopidy.connected
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Playlist)
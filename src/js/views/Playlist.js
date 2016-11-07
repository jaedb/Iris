
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
		}else if( !this.props.mopidy.connected && nextProps.mopidy.connected ){
			if( helpers.uriSource( this.props.params.uri ) == 'm3u' ){
				this.loadPlaylist( nextProps )
			}
		}
	}

	loadPlaylist( props = this.props ){
		var source = helpers.uriSource( props.params.uri );
		if( source == 'spotify' ){
			this.props.spotifyActions.getPlaylist( props.params.uri );
		}else if( source == 'm3u' && props.mopidy.connected ){
			this.props.mopidyActions.getPlaylist( props.params.uri );
		}
	}

	loadMore(){
		if( !this.props.spotify.playlist || !this.props.spotify.playlist.tracks.next ) return
		this.props.spotifyActions.getURL( this.props.spotify.playlist.tracks.next, 'SPOTIFY_PLAYLIST_LOADED_MORE' );
	}

	render(){
		var source = helpers.uriSource( this.props.params.uri );
		var source_name = source
		if( source == 'spotify' ){
			var playlist = this.props.spotify.playlist
		}else if( source == 'm3u' ){
			var playlist = this.props.mopidy.playlist
			source_name = 'local'
		}
		if( !playlist ) return null;

		return (
			<div className="view playlist-view">
				<div className="intro">
					{ playlist.images ? <Thumbnail size="large" images={ playlist.images } /> : null }
					<ul className="details">
						<li>{ playlist.tracks.total } tracks, <Dater type="total-time" data={playlist.tracks.items} /></li>
						{ playlist.last_modified ? <li>Updated <Dater type="ago" data={playlist.last_modified} /> ago</li> : null }
						<li><FontAwesome name={helpers.sourceIcon( this.props.params.uri )} /> {source_name} playlist</li>
					</ul>
				</div>
				<div className="main">

					<div className="title">
						<h1>{ playlist.name }</h1>
					</div>

					<section className="list-wrapper">
						<TrackList tracks={ playlist.tracks.items } />
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
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Playlist)
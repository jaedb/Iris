
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
let helpers = require('../helpers.js')

import TrackList from '../components/TrackList'
import Thumbnail from '../components/Thumbnail'
import Parallax from '../components/Parallax'
import ArtistSentence from '../components/ArtistSentence'
import ArtistGrid from '../components/ArtistGrid'
import Dater from '../components/Dater'

import * as spotifyActions from '../services/spotify/actions'
import * as mopidyActions from '../services/mopidy/actions'

class Album extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.loadAlbum();
	}

	componentWillReceiveProps( nextProps ){
		if( nextProps.params.uri != this.props.params.uri ){
			this.loadAlbum( nextProps )
		}else if( !this.props.mopidy.connected && nextProps.mopidy.connected ){
			if( helpers.uriSource( this.props.params.uri ) == 'local' ){
				this.loadAlbum( nextProps )
			}
		}
	}

	loadAlbum( props = this.props ){
		var source = helpers.uriSource( props.params.uri );
		if( source == 'spotify' ){
			this.props.spotifyActions.getAlbum( props.params.uri );
		}else if( source == 'local' && props.mopidy.connected ){
			this.props.mopidyActions.getAlbum( props.params.uri );
		}
	}

	totalTime(){
		if( !this.props.spotify.album ) return null
		var tracks = this.props.spotify.tracks.items;
		var duration = 0;
		for( var i = 0; i < tracks.length; i++ ){
			duration += tracks[i].duration_ms;
		}
		return duration;
	}

	render(){
		var source = helpers.uriSource( this.props.params.uri );
		if( source == 'spotify' ) var album = this.props.spotify.album
		if( source == 'local' ) var album = this.props.mopidy.album
		if( !album ) return null;

		return (
			<div className="view album-view">
				<div className="intro">
					<Thumbnail size="large" images={ album.images } />
					<ArtistGrid artists={ album.artists } />
					<div className="details">
						<div>{ album.tracks.total } tracks, { this.totalTime }</div>
						<div>Released <Dater type="date" data={ album.release_date } /></div>
					</div>
				</div>
				<div className="main">
					<div className="title">
						<h1>{ album.name }</h1>
						<h3><ArtistSentence artists={ album.artists } /></h3>
					</div>
					<TrackList tracks={ album.tracks.items } />
				</div>
			</div>
		);
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

export default connect(mapStateToProps, mapDispatchToProps)(Album)

import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import TrackList from '../components/TrackList'
import Thumbnail from '../components/Thumbnail'
import Parallax from '../components/Parallax'
import ArtistSentence from '../components/ArtistSentence'

import * as spotifyActions from '../services/spotify/actions'
import * as mopidyActions from '../services/mopidy/actions'

class Album extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.props.spotifyActions.getAlbum( this.props.params.uri );
	}

	// when props changed
	componentWillReceiveProps( nextProps ){
		if( nextProps.params.uri != this.props.params.uri ){
			this.props.spotifyActions.getAlbum( nextProps.params.uri );
		}
	}

	render(){
		if( this.props.spotify.album ){
			return (
				<div className="view album-view">
					<div className="intro">
						<Thumbnail size="large" images={ this.props.spotify.album.images } />
					</div>
					<div className="main">
						<h1>{ this.props.spotify.album.name }</h1>
						<h3><ArtistSentence artists={ this.props.spotify.album.artists } /></h3>
						<TrackList tracks={this.props.spotify.album.tracks.items} />
					</div>
				</div>
			);
		}
		return null;
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
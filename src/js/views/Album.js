
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import TrackList from '../components/TrackList'
import * as spotifyActions from '../services/spotify/actions'
import * as mopidyActions from '../services/mopidy/actions'

class Album extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.props.spotifyActions.loadAlbum( this.props.params.uri );
	}

	// when props changed
	componentWillReceiveProps( nextProps ){
		if( nextProps.params.uri != this.props.params.uri ){
			this.props.spotifyActions.loadAlbum( nextProps.params.uri );
		}
	}

	playTracks( tracks ){
		var uris = [];
		for( var i = 0; i < tracks.length; i++ ){
			uris.push( tracks[i].uri )
		}
		this.props.mopidyActions.playTracks( uris )
	}

	playTrack( track ){		
		var uris = [track.uri];
		this.props.mopidyActions.playTracks( uris )
	}

	render(){
		if( this.props.spotify.album ){
			return (
				<div>
					<h3>{ this.props.spotify.album.name }</h3>
					<TrackList
						tracks={this.props.spotify.album.tracks.items} 
						removeTracks={ null }
						playTracks={ tracks => this.playTracks( tracks ) }
						playTrack={ track => this.playTrack( track ) }
						/>
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
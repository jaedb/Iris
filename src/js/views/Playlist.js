
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import TrackList from '../components/TrackList'
import Header from '../components/Header'

import * as spotifyActions from '../services/spotify/actions'
import * as mopidyActions from '../services/mopidy/actions'

class Playlist extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.props.spotifyActions.getPlaylist( this.props.params.uri );
	}

	// when props changed
	componentWillReceiveProps( nextProps ){
		if( nextProps.params.uri != this.props.params.uri ){
			this.props.spotifyActions.getPlaylist( nextProps.params.uri );
		}
	}

	render(){
		if( this.props.spotify.playlist ){
			
			var context = null;
			if( this.props.spotify.playlist.owner.id == this.props.spotify.me.id ) context = 'editable-playlist'

			return (
				<div className="view playlist-view">
					<Header icon="playlist" title={ this.props.spotify.playlist.name } />
					<TrackList context={context} tracks={this.props.spotify.playlist.tracks.items} />
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

export default connect(mapStateToProps, mapDispatchToProps)(Playlist)
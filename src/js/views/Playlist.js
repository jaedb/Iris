
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import TrackList from '../components/TrackList'
import Thumbnail from '../components/Thumbnail'
import Dater from '../components/Dater'

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
			
			var playlist = this.props.spotify.playlist;
			var context = null;
			if( playlist.owner.id == this.props.spotify.me.id ) context = 'editable-playlist'

			return (
				<div className="view playlist-view">
					<div className="intro">
						<Thumbnail size="large" images={ playlist.images } />
						<div className="details">
							<div>{ playlist.tracks.total } tracks</div>
						</div>
					</div>
					<div className="main">
						<div className="title">
							<h1>{ playlist.name }</h1>
						</div>
						<TrackList context={context} tracks={ playlist.tracks.items } />
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

export default connect(mapStateToProps, mapDispatchToProps)(Playlist)
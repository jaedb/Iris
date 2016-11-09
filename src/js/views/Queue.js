
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import FontAwesome from 'react-fontawesome'
import TrackList from '../components/TrackList'
import Track from '../components/Track'
import FullPlayer from '../components/FullPlayer'
import ArtistSentence from '../components/ArtistSentence'
import AlbumLink from '../components/AlbumLink'
import Header from '../components/Header'

import * as uiActions from '../services/ui/actions'
import * as spotifyActions from '../services/spotify/actions'
import * as mopidyActions from '../services/mopidy/actions'

class Queue extends React.Component{

	constructor(props) {
		super(props);
	}

	removeTracks( tracks ){
		var tlids = [];
		for( var i = 0; i < tracks.length; i++ ){
			tlids.push( tracks[i].tlid )
		}
		this.props.mopidyActions.removeTracks( tlids )
	}

	playTrack( track ){
		this.props.mopidyActions.changeTrack( track.tlid )
	}

	playTracks( tracks ){
		this.props.mopidyActions.changeTrack( tracks[0].tlid )
	}

	render(){
		return (
			<div className="view queue-view">

				<FullPlayer />

				<section className="list-wrapper">
					<TrackList
						show_source_icon={true}
						context="queue"
						tracks={this.props.current_tracklist} 
						removeTracks={ tracks => this.removeTracks( tracks ) }
						playTracks={ tracks => this.playTracks( tracks ) }
						playTrack={ track => this.playTrack( track ) } />
				</section>
				
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
	return {
		current_tracklist: state.ui.current_tracklist
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Queue)
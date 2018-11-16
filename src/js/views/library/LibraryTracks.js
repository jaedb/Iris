
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import Link from '../../components/Link';

import TrackList from '../../components/TrackList'
import Header from '../../components/Header'
import LazyLoadListener from '../../components/LazyLoadListener'
import Icon from '../../components/Icon'

import * as helpers from '../../helpers'
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryTracks extends React.Component{

	constructor(props){
		super(props);
	}

	// on render
	componentDidMount(){
		this.props.uiActions.setWindowTitle("Tracks");

		if (!this.props.spotify_me){
			this.props.uiActions.createNotification({type: 'warning', content: 'Enable Spotify to browse tracks'});
		} else if (this.props.library_tracks === undefined){
			this.props.spotifyActions.getLibraryTracks();
		}
	}

	loadMore(){
		this.props.spotifyActions.getMore(
			this.props.library_tracks_more,
			null,
			{
				type: 'SPOTIFY_LIBRARY_TRACKS_LOADED_MORE'
			}
		);
	}

	playAll(){
		this.props.spotifyActions.getLibraryTracksAndPlay();
	}

	render(){
		if (helpers.isLoading(this.props.load_queue,['spotify_me/tracks'])){
			return (
				<div className="view library-tracks-view">
					<Header icon="music" title="My tracks" />
					<div className="body-loader loading">
						<div className="loader"></div>
					</div>
				</div>
			)
		}

		var tracks = [];
		if (this.props.library_tracks && this.props.tracks){
			for (var i = 0; i < this.props.library_tracks.length; i++){
				var uri = this.props.library_tracks[i]
				if (this.props.tracks.hasOwnProperty(uri)){
					tracks.push(this.props.tracks[uri])
				}
			}
		}

		var options = (
			<a className="button button--no-hover" onClick={e => this.playAll(e)}>
				<Icon name="play_circle_filled" />Play all
			</a>
		);

		return (
			<div className="view library-tracks-view">
				<Header options={options} uiActions={this.props.uiActions}>
					<Icon name="music_note" type="material" />
					My tracks
				</Header>
				<section className="content-wrapper">
					<TrackList tracks={tracks} />
					<LazyLoadListener 
						loadKey={this.props.library_tracks_more}
						showLoader={this.props.library_tracks_more}
						loadMore={() => this.loadMore()}
					/>
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
		load_queue: state.ui.load_queue,
		tracks: state.core.tracks,
		spotify_me: state.spotify.me,
		library_tracks: state.spotify.library_tracks,
		library_tracks_more: state.spotify.library_tracks_more
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryTracks)
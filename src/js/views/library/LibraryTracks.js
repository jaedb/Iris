
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import TrackList from '../../components/TrackList'
import Header from '../../components/Header'
import LazyLoadListener from '../../components/LazyLoadListener'

import * as helpers from '../../helpers'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryTracks extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.props.spotifyActions.getLibraryTracks();
	}

	loadMore(){
		this.props.spotifyActions.getURL( this.props.tracks_more, 'SPOTIFY_LIBRARY_TRACKS_LOADED_MORE' );
	}

	render(){
		if (helpers.isLoading(this.props.load_queue,['spotify_me/tracks'])){
			return (
				<div className="view library-tracks-view">
					<Header icon="music" title="My tracks" />
					<div className="body-loader">
						<div className="loader"></div>
					</div>
				</div>
			)
		}

		return (
			<div className="view library-tracks-view">
				<Header icon="music" title="My tracks" />
				<section className="list-wrapper">
					{ this.props.tracks ? <TrackList tracks={this.props.tracks} /> : null }
					<LazyLoadListener enabled={this.props.tracks_more} loadMore={ () => this.loadMore() }/>
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
		tracks: state.spotify.library_tracks,
		tracks_more: state.spotify.library_tracks_more
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryTracks)

import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import TrackList from '../../components/TrackList'
import Header from '../../components/Header'
import LazyLoadListener from '../../components/LazyLoadListener'

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
		if( !this.props.tracks_more ) return
		this.props.spotifyActions.getURL( this.props.tracks_more, 'SPOTIFY_LIBRARY_TRACKS_LOADED_MORE' );
	}

	render(){
		return (
			<div className="view library-tracks-view">
				<Header icon="music" title="My tracks" />
				<section className="list-wrapper">
					{ this.props.tracks ? <TrackList tracks={this.props.tracks} /> : null }
					<LazyLoadListener loadMore={ () => this.loadMore() }/>
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
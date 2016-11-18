
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import LazyLoadListener from '../../components/LazyLoadListener'
import Header from '../../components/Header'
import ArtistGrid from '../../components/ArtistGrid'

import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryArtists extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.props.spotifyActions.getLibraryArtists();
	}

	loadMore(){
		if( !this.props.spotify.library_artists_more ) return
		this.props.spotifyActions.getURL( this.props.spotify.library_artists_more, 'SPOTIFY_LIBRARY_ARTISTS_LOADED_MORE' );
	}

	render(){
		return (
			<div className="view library-artists-view">
				<Header icon="mic" title="My artists" />
				<section className="grid-wrapper">
					{ this.props.spotify.library_artists ? <ArtistGrid artists={this.props.spotify.library_artists} /> : null }
				</section>
				<LazyLoadListener loadMore={ () => this.loadMore() }/>
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

export default connect(mapStateToProps, mapDispatchToProps)(LibraryArtists)
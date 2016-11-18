
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import AlbumGrid from '../../components/AlbumGrid'
import Header from '../../components/Header'
import LazyLoadListener from '../../components/LazyLoadListener'

import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryAlbums extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.props.spotifyActions.getLibraryAlbums();
	}

	loadMore(){
		if( !this.props.spotify.library_albums_more ) return
		this.props.spotifyActions.getURL( this.props.spotify.library_albums_more, 'SPOTIFY_LIBRARY_ALBUMS_LOADED_MORE' );
	}

	render(){
		return (
			<div className="view library-albums-view">
				<Header
					icon="cd"
					title="My albums"
					/>
				<section className="grid-wrapper">
					{ this.props.spotify.library_albums ? <AlbumGrid albums={this.props.spotify.library_albums} /> : null }
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

export default connect(mapStateToProps, mapDispatchToProps)(LibraryAlbums)
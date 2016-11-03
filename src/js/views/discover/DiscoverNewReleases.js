
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Header from '../../components/Header'
import AlbumGrid from '../../components/AlbumGrid'
import LazyLoadListener from '../../components/LazyLoadListener'

import * as uiActions from '../../services/ui/actions'
import * as spotifyActions from '../../services/spotify/actions'

class DiscoverNewReleases extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.props.spotifyActions.getNewReleases();
	}

	loadMore(){
		if( !this.props.spotify.new_releases || !this.props.spotify.new_releases.next ) return
		this.props.spotifyActions.getURL( this.props.spotify.new_releases.next, 'SPOTIFY_NEW_RELEASES_LOADED_MORE' );
	}

	render(){
		return (
			<div className="view discover-new-releases-view">
				<Header icon="leaf" title="New Releases" />
				<section className="grid-wrapper">
					{ this.props.spotify.new_releases ? <AlbumGrid albums={this.props.spotify.new_releases.items} /> : null }
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
		uiActions: bindActionCreators(uiActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverNewReleases)
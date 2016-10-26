
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Header from '../../components/Header'
import PlaylistGrid from '../../components/PlaylistGrid'

import * as spotifyActions from '../../services/spotify/actions'

class DiscoverFeatured extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.props.spotifyActions.getFeaturedPlaylists();
	}

	render(){
		return (
			<div className="view discover-featured-view">
				<Header icon="star" title="Featured playlists" />
				{ this.props.spotify.featuredPlaylists ? <PlaylistGrid playlists={this.props.spotify.featuredPlaylists.playlists.items} /> : null }
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
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverFeatured)
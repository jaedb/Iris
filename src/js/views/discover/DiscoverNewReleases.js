
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Header from '../../components/Header'
import AlbumGrid from '../../components/AlbumGrid'

import * as spotifyActions from '../../services/spotify/actions'

class DiscoverNewReleases extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.props.spotifyActions.getNewReleases();
	}

	render(){
		return (
			<div className="view discover-new-releases-view">
				<Header icon="leaf" title="New Releases" />
				{ this.props.spotify.new_releases ? <AlbumGrid albums={this.props.spotify.new_releases.items} /> : null }
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

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverNewReleases)
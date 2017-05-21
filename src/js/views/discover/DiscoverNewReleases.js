
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Header from '../../components/Header'
import AlbumGrid from '../../components/AlbumGrid'
import LazyLoadListener from '../../components/LazyLoadListener'
import * as helpers from '../../helpers'
import * as uiActions from '../../services/ui/actions'
import * as spotifyActions from '../../services/spotify/actions'

class DiscoverNewReleases extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		if (!this.props.new_releases) this.props.spotifyActions.getNewReleases();
	}

	loadMore(){
		this.props.spotifyActions.getURL(this.props.new_releases_more, 'SPOTIFY_NEW_RELEASES_LOADED');
	}

	render(){
		if (helpers.isLoading(this.props.load_queue,['spotify_browse/new-releases'])){
			return (
				<div className="view discover-new-releases-view">
					<Header icon="leaf" title="New Releases" />
					<div className="body-loader">
						<div className="loader"></div>
					</div>
				</div>
			)
		}

		var albums = []
		if (this.props.new_releases){
			for (var i = 0; i < this.props.new_releases.length; i++){
				var uri = this.props.new_releases[i]
				if (this.props.albums.hasOwnProperty(uri)){
					albums.push(this.props.albums[uri])
				}
			}
		}

		return (
			<div className="view discover-new-releases-view">
				<Header icon="leaf" title="New Releases" />
				<section className="grid-wrapper">
					<AlbumGrid albums={albums} />
				</section>
				<LazyLoadListener enabled={this.props.new_releases_more} loadMore={ () => this.loadMore() }/>
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
		albums: state.ui.albums,
		new_releases: state.ui.new_releases,
		new_releases_more: state.ui.new_releases_more,
		new_releases_total: state.ui.new_releases_total
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverNewReleases)
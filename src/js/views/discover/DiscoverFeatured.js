
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import PlaylistGrid from '../../components/PlaylistGrid'
import SidebarToggleButton from '../../components/SidebarToggleButton'
import Parallax from '../../components/Parallax'
import Thumbnail from '../../components/Thumbnail'
import * as helpers from '../../helpers'
import * as spotifyActions from '../../services/spotify/actions'

class DiscoverFeatured extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.props.spotifyActions.getFeaturedPlaylists();
	}

	renderIntro(playlist = null){
		if (playlist){
			return (
				<div className="intro">
					<Parallax image={helpers.sizedImages(playlist.images).huge} />
					<div className="content cf">
						<Thumbnail images={playlist.images} />
						<h1>{playlist.name}</h1>
						<h3>{playlist.tracks_total} tracks</h3>
					</div>
				</div>
			)
		} else {
			return (
				<div className="intro">
					<Parallax />
				</div>
			)
		}
	}

	render(){
		if (helpers.isLoading(this.props.load_queue,['spotify_browse/featured-playlists'])){
			return (
				<div className="view discover-featured-view">
					<div className="body-loader">
						<div className="loader"></div>
					</div>
				</div>
			)
		}

		var playlists = []
		if (this.props.featured_playlists){
			for (var i = 0; i < this.props.featured_playlists.playlists.length; i++){
				var uri = this.props.featured_playlists.playlists[i]
				if (this.props.playlists.hasOwnProperty(uri)){
					playlists.push(this.props.playlists[uri])
				}
			}
		}

		// Pull the first playlist out and we'll use this as a banner
		var first_playlist = playlists.splice(0,1)
		if (first_playlist){
			first_playlist = first_playlist[0]
		}

		return (
			<div className="view discover-featured-view">
				<SidebarToggleButton />
				{this.renderIntro(first_playlist)}
				<section className="grid-wrapper">
					{playlists ? <PlaylistGrid playlists={playlists} /> : null }
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
		featured_playlists: state.spotify.featured_playlists,
		playlists: state.ui.playlists
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverFeatured)
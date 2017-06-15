
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import Icon from './Icon'
import SearchForm from './SearchForm'
import Dropzones from './Dropzones'

import FontAwesome from 'react-fontawesome'
import * as mopidyActions from '../services/mopidy/actions'

class Sidebar extends React.Component{

	constructor(props) {
		super(props)
	}

	render(){
		return (
			<aside>

				<div className="liner">

		        	<SearchForm context="sidebar" />

		        	<nav>

		        		<section>
							<Link activeClassName="active" to={global.baseURL+"queue"}>
								<Icon name="play" />
								Now playing
							</Link>
						</section>

						<section>
							<title>Discover</title>
							<Link activeClassName="active" to={global.baseURL+"discover/recommendations"}>
								<Icon name="compass" />
								Discover
							</Link>
							<Link activeClassName="active" to={global.baseURL+"discover/categories"}>
								<Icon name="grid" />
								Genre / Mood
							</Link>
							<Link activeClassName="active" to={global.baseURL+"discover/featured"}>
								<Icon name="star" />
								Featured playlists
							</Link>
							<Link activeClassName="active" to={global.baseURL+"discover/new-releases"}>
								<Icon name="leaf" />
								New releases
							</Link>
						</section>

						<section>
							<title>My Music</title>
							<Link activeClassName="active" to={global.baseURL+"library/playlists"}>
								<Icon name="playlist" />
								Playlists
							</Link>
							<Link activeClassName="active" disabled={!this.props.spotify_authorized} to={this.props.spotify_authorized ? global.baseURL+"library/artists" : null}>
								<Icon name="mic" />
								Artists
							</Link>
							<Link activeClassName="active" disabled={!this.props.spotify_authorized} to={this.props.spotify_authorized ? global.baseURL+"library/albums" : null}>
								<Icon name="cd" />
								Albums
							</Link>
							<Link activeClassName="active" disabled={!this.props.spotify_authorized} to={this.props.spotify_authorized ? global.baseURL+"library/tracks" : null}>
								<Icon name="music" />
								Tracks
							</Link>
							<Link activeClassName="active" to={global.baseURL+"library/local"}>
								<Icon name="folder" />
								Local
							</Link>
						</section>

						<section>
							<Link activeClassName="active" to={global.baseURL+"settings"}>
								<Icon name="cog" />
								Settings
								{ !this.props.mopidy_connected || !this.props.spotify_connected || !this.props.pusher_connected ? <FontAwesome name="exclamation-triangle" className="red-text pull-right" /> : null }
							</Link>
						</section>

			        </nav>

			    </div>

		       	<Dropzones />

			</aside>
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
		mopidy_connected: state.mopidy.connected,
		pusher_connected: state.pusher.connected,
		spotify_connected: state.spotify.connected,
		spotify_authorized: state.spotify.authorized,
		dragger: state.ui.dragger
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Sidebar)
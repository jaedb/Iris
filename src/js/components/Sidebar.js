
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link, withRouter } from 'react-router'

import Icon from './Icon'
import Dropzones from './Dropzones'
import Thumbnail from './Thumbnail'

import FontAwesome from 'react-fontawesome'
import * as mopidyActions from '../services/mopidy/actions'

class Sidebar extends React.Component{

	constructor(props) {
		super(props)
	}

	linkClassName(link){
		if (this.props.location.pathname.startsWith('/'+link)){
			return 'active'
		} else {
			return null
		}
	}

	render(){
		return (
			<aside>
				<div className="liner">
		        	<nav>

		        		<section>
							<Link className={this.linkClassName('queue')} to={global.baseURL+"queue"}>
								<Icon name="play" />
								Now playing
							</Link>
							<Link className={this.linkClassName('search')} to={global.baseURL+"search"}>
								<Icon name="search" />
								Search
							</Link>
						</section>

						{this.props.spotify_enabled ? <section>
							<title>Discover</title>
							<Link className={this.linkClassName('discover/recommendations')} to={global.baseURL+"discover/recommendations"}>
								<Icon name="compass" />
								Discover
							</Link>
							<Link className={this.linkClassName('discover/categories')} to={global.baseURL+"discover/categories"}>
								<Icon name="grid" />
								Genre / Mood
							</Link>
							<Link className={this.linkClassName('discover/featured')} to={global.baseURL+"discover/featured"}>
								<Icon name="star" />
								Featured playlists
							</Link>
							<Link className={this.linkClassName('discover/new-releases')} to={global.baseURL+"discover/new-releases"}>
								<Icon name="leaf" />
								New releases
							</Link>
						</section> : null}

						{this.props.ais_enabled &&
						<section>
							<title>AI-Speaker</title>
							<Link className={this.linkClassName('library/browse/ais:root:1')} to={global.baseURL+"library/browse/ais:root:1"}>
								<Icon name="ais" />
								Discover
							</Link>
							<Link className={this.linkClassName('library/playlists')} to={global.baseURL+"library/playlists"}>
								<Icon name="playlist" />
								Playlists
							</Link>
							<Link className={this.linkClassName('library/browse/ais:root:2')} to={global.baseURL+"library/browse/ais:root:2"}>
								<Icon name="ais-library" />
								My Library
							</Link>
							<Link className={this.linkClassName('library/browse/ais:root:3')} to={global.baseURL+"library/browse/ais:root:3"}>
								<Icon name="ais-family" />
								My Family
							</Link>
							<Link className={this.linkClassName('library/browse/ais:root:4')} to={global.baseURL+"library/browse/ais:root:4"}>
								<Icon name="ais-likes" />
								My Likes
							</Link>
							<Link className={this.linkClassName('library/browse/ais:root:5')} to={global.baseURL+"library/browse/ais:root:5"}>
								<Icon name="ais-pendrive" />
								Pendrive
							</Link>
						</section>}

						{!this.props.ais_enabled &&
						<section>
							<title>My Music</title>
							<Link className={this.linkClassName('library/playlists')} to={global.baseURL+"library/playlists"}>
								<Icon name="playlist" />
								Playlists
							</Link>
							<Link className={this.linkClassName('library/artists')} to={global.baseURL+"library/artists"}>
								<Icon name="mic" />
								Artists
							</Link>
							<Link className={this.linkClassName('library/albums')} to={global.baseURL+"library/albums"}>
								<Icon name="cd" />
								Albums
							</Link>
							<Link className={this.linkClassName('library/tracks')} to={global.baseURL+"library/tracks"}>
								<Icon name="cd" />
								Tracks
							</Link>
							<Link className={this.linkClassName('library/browse')} to={global.baseURL+"library/browse"}>
								<Icon name="folder" />
								Browse
							</Link>
						</section>}

						<section>
							<Link className={this.linkClassName('settings')} to={global.baseURL+"settings"}>
								<Icon name="cog" />
								Settings
								{ !this.props.mopidy_connected || (!this.props.spotify_connected && this.props.spotify_enabled) || !this.props.pusher_connected ? <FontAwesome name="exclamation-triangle" className="red-text pull-right" /> : null }
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
		spotify_enabled: state.spotify.enabled,
		spotify_connected: state.spotify.connected,
		spotify_authorized: state.spotify.authorization,
		dragger: state.ui.dragger,
		ais_enabled: state.mopidy.uri_schemes.indexOf('ais:') > -1
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

// We wrap our Sidebar with the Router, and then to the redux store
export default connect(mapStateToProps, mapDispatchToProps)(withRouter(Sidebar))

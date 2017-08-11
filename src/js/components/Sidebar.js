
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
							<Link className={this.linkClassName('library/browse')} to={global.baseURL+"library/browse"}>
								<Icon name="folder" />
								Browse
							</Link>
						</section>

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
		dragger: state.ui.dragger
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

// We wrap our Sidebar with the Router, and then to the redux store
export default connect(mapStateToProps, mapDispatchToProps)(withRouter(Sidebar))
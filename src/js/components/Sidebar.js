
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link, withRouter } from 'react-router'

import Icon from './Icon'
import Dropzones from './Fields/Dropzones'
import Thumbnail from './Thumbnail'

import FontAwesome from 'react-fontawesome'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'

class Sidebar extends React.Component{

	constructor(props){
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
								<Icon name="play_arrow" type="material" />
								Now playing
							</Link>
							<Link className={this.linkClassName('search')} to={global.baseURL+"search"}>
								<Icon name="search" type="material" />
								Search
							</Link>
						</section>

						{this.props.spotify_enabled ? <section>
							<title>Discover</title>
							<Link className={this.linkClassName('discover/recommendations')} to={global.baseURL+"discover/recommendations"}>
								<Icon name="explore" type="material" />
								Discover
							</Link>
							<Link className={this.linkClassName('discover/categories')} to={global.baseURL+"discover/categories"}>
								<Icon name="mood" type="material" />
								Genre / Mood
							</Link>
							<Link className={this.linkClassName('discover/featured')} to={global.baseURL+"discover/featured"}>
								<Icon name="star" type="material" />
								Featured playlists
							</Link>
							<Link className={this.linkClassName('discover/new-releases')} to={global.baseURL+"discover/new-releases"}>
								<Icon name="new_releases" type="material" />
								New releases
							</Link>
						</section> : null}

						<section>
							<title>My Music</title>
							<Link className={this.linkClassName('library/playlists')} to={global.baseURL+"library/playlists"}>
								<Icon name="queue_music" type="material" />
								Playlists
							</Link>
							<Link className={this.linkClassName('library/artists')} to={global.baseURL+"library/artists"}>
								<Icon name="recent_actors" type="material" />
								Artists
							</Link>
							<Link className={this.linkClassName('library/albums')} to={global.baseURL+"library/albums"}>
								<Icon name="album" type="material" />
								Albums
							</Link>
							<Link className={this.linkClassName('library/tracks')} to={global.baseURL+"library/tracks"}>
								<Icon name="music_note" type="material" />
								Tracks
							</Link>
							<Link className={this.linkClassName('library/browse')} to={global.baseURL+"library/browse"}>
								<Icon name="folder" type="material" />
								Browse
							</Link>
						</section>

						<section>
							<Link className={this.linkClassName('settings')} to={global.baseURL+"settings"}>
								<Icon name="settings" type="material" />
								Settings
								{this.props.test_mode ? <span className="status has-tooltip right-tooltip"><Icon name="info" className="orange-text" /><span className="tooltip">Test mode active</span></span>: null}
								{!this.props.mopidy_connected || !this.props.pusher_connected ? <span className="status has-tooltip right-tooltip"><Icon name="warning" className="red-text" /><span className="tooltip">{!this.props.mopidy_connected ? <span>Mopidy not connected<br /></span> : null}{!this.props.pusher_connected ? <span>Pusher not connected<br /></span> : null}</span></span> : null}
							</Link>
						</section>

			        </nav>
			    </div>

		       	<Dropzones />

		       	<div className="close" onClick={e => this.props.uiActions.toggleSidebar(false)}>
		       		<FontAwesome name="chevron-right" />
		       	</div>

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
		spotify_authorized: state.spotify.authorization,
		test_mode: (state.ui.test_mode ? state.ui.test_mode : false),
		dragger: state.ui.dragger
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

// We wrap our Sidebar with the Router, and then to the redux store
export default connect(mapStateToProps, mapDispatchToProps)(withRouter(Sidebar))

import React, { PropTypes } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { withRouter } from 'react-router';

import Link from './Link';
import Icon from './Icon';
import Dropzones from './Fields/Dropzones';
import Thumbnail from './Thumbnail';

import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';

class Sidebar extends React.Component{

	constructor(props){
		super(props)
	}

	render(){
		return (
			<aside>
				<div className="liner">
		        	<nav>

		        		<section>
							<Link nav to={global.baseURL+"queue"}>
								<Icon name="play_arrow" type="material" />
								Now playing
							</Link>
							<Link nav to={global.baseURL+"search"}>
								<Icon name="search" type="material" />
								Search
							</Link>
						</section>

						{this.props.spotify_enabled ? <section>
							<title>Discover</title>
							<Link nav to={global.baseURL+"discover/recommendations"}>
								<Icon name="explore" type="material" />
								Discover
							</Link>
							<Link nav to={global.baseURL+"discover/categories"}>
								<Icon name="mood" type="material" />
								Genre / Mood
							</Link>
							<Link nav to={global.baseURL+"discover/featured"}>
								<Icon name="star" type="material" />
								Featured playlists
							</Link>
							<Link nav to={global.baseURL+"discover/new-releases"}>
								<Icon name="new_releases" type="material" />
								New releases
							</Link>
						</section> : null}

						<section>
							<title>My Music</title>
							<Link nav to={global.baseURL+"library/playlists"}>
								<Icon name="queue_music" type="material" />
								Playlists
							</Link>
							<Link nav to={global.baseURL+"library/artists"}>
								<Icon name="recent_actors" type="material" />
								Artists
							</Link>
							<Link nav to={global.baseURL+"library/albums"}>
								<Icon name="album" type="material" />
								Albums
							</Link>
							<Link nav to={global.baseURL+"library/tracks"}>
								<Icon name="music_note" type="material" />
								Tracks
							</Link>
							<Link nav to={global.baseURL+"library/browse"}>
								<Icon name="folder" type="material" />
								Browse
							</Link>
						</section>

						<section>
							<Link nav to={global.baseURL+"settings"}>
								<Icon name="settings" type="material" />
								Settings
								{this.props.update_available ? <span className="status tooltip tooltip--right"><Icon name="cloud_download" className="green-text" /><span className="tooltip__content">Update available</span></span>: null}
								{!this.props.mopidy_connected || !this.props.pusher_connected ? <span className="status tooltip tooltip--right"><Icon name="warning" className="red-text" /><span className="tooltip__content">{!this.props.mopidy_connected ? <span>Mopidy not connected<br /></span> : null}{!this.props.pusher_connected ? <span>Pusher not connected<br /></span> : null}</span></span> : null}
							</Link>
						</section>

			        </nav>
			    </div>

		       	<Dropzones />

		       	<div className="close" onClick={e => this.props.uiActions.toggleSidebar(false)}>
		       		<Icon name="close" />
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
		update_available: (state.pusher.version && state.pusher.version.update_available ? state.pusher.version.update_available : false),
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

export default connect(mapStateToProps, mapDispatchToProps)(withRouter(Sidebar));
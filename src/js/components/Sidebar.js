
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
			<aside className="sidebar">
				<div className="sidebar__liner">
		        	<nav className="sidebar__menu">

		        		<section className="sidebar__menu__section">
							<Link to={"/queue"} history={this.props.history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
								<Icon name="play_arrow" type="material" />
								Now playing
							</Link>
							<Link to={"/search"} history={this.props.history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
								<Icon name="search" type="material" />
								Search
							</Link>
						</section>

						{this.props.spotify_enabled ? <section className="sidebar__menu__section">
							<title className="sidebar__menu__section__title">Discover</title>
							<Link to={"/discover/recommendations"} history={this.props.history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
								<Icon name="explore" type="material" />
								Discover
							</Link>
							<Link to={"/discover/categories"} history={this.props.history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
								<Icon name="mood" type="material" />
								Genre / Mood
							</Link>
							<Link to={"/discover/featured"} history={this.props.history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
								<Icon name="star" type="material" />
								Featured playlists
							</Link>
							<Link to={"/discover/new-releases"} history={this.props.history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
								<Icon name="new_releases" type="material" />
								New releases
							</Link>
						</section> : null}

						<section className="sidebar__menu__section">
							<title className="sidebar__menu__section__title">My Music</title>
							<Link to={"/library/playlists"} history={this.props.history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
								<Icon name="queue_music" type="material" />
								Playlists
							</Link>
							<Link to={"/library/artists"} history={this.props.history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
								<Icon name="recent_actors" type="material" />
								Artists
							</Link>
							<Link to={"/library/albums"} history={this.props.history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
								<Icon name="album" type="material" />
								Albums
							</Link>
							<Link to={"/library/tracks"} history={this.props.history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
								<Icon name="music_note" type="material" />
								Tracks
							</Link>
							<Link to={"/library/browse"} history={this.props.history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
								<Icon name="folder" type="material" />
								Browse
							</Link>
						</section>

						<section className="sidebar__menu__section">
							<Link to={"/settings"} history={this.props.history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
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
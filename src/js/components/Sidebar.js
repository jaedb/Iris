
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { withRouter } from 'react-router';
import Link from './Link';
import Icon from './Icon';
import Dropzones from './Fields/Dropzones';
import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';

class Sidebar extends React.Component {
  renderStatusIcon() {
    const {
      update_available,
      mopidy_connected,
      pusher_connected,
      snapcast_connected,
      snapcast_enabled,
    } = this.props;
    if (update_available) {
      return (
        <span className="status tooltip tooltip--right">
          <Icon name="cloud_download" className="green-text" />
          <span className="tooltip__content">Update available</span>
        </span>
      );
    }

    if (!navigator.onLine) {
      return (
        <span className="status tooltip tooltip--right">
          <Icon name="wifi_off" className="red-text" />
          <span className="tooltip__content">
            Browser offline
          </span>
        </span>
      );
    }

    if (!mopidy_connected || !pusher_connected || (!snapcast_connected && snapcast_enabled)) {
      return (
        <span className="status tooltip tooltip--right">
          <Icon name="warning" className="red-text" />
          <span className="tooltip__content">
            {!mopidy_connected && (<span>Mopidy not connected<br /></span>)}
            {!pusher_connected && (<span>Pusher not connected<br /></span>)}
            {!snapcast_connected && snapcast_enabled && (<span>Snapcast not connected<br /></span>)}
          </span>
        </span>
      );
    }

    return null;
  }

  render() {
    const {
      history,
      spotify_available,
      uiActions,
    } = this.props;

    return (
      <aside className="sidebar">
        <div className="sidebar__liner">
          <nav className="sidebar__menu">

            <section className="sidebar__menu__section">
              <Link to="/queue" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="play_arrow" type="material" />
								Now playing
              </Link>
              <Link to="/search" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="search" type="material" />
								Search
              </Link>
            </section>

            {spotify_available && (
              <section className="sidebar__menu__section">
                <title className="sidebar__menu__section__title">Discover</title>
                <Link to="/discover/recommendations" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                  <Icon name="explore" type="material" />
								Discover
                </Link>
                <Link to="/discover/categories" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                  <Icon name="mood" type="material" />
								Genre / Mood
                </Link>
                <Link to="/discover/featured" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                  <Icon name="star" type="material" />
								Featured playlists
                </Link>
                <Link to="/discover/new-releases" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                  <Icon name="new_releases" type="material" />
								New releases
                </Link>
              </section>
            )}

            <section className="sidebar__menu__section">
              <title className="sidebar__menu__section__title">My Music</title>
              <Link to="/library/playlists" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="queue_music" type="material" />
								Playlists
              </Link>
              <Link to="/library/artists" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="recent_actors" type="material" />
								Artists
              </Link>
              <Link to="/library/albums" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="album" type="material" />
								Albums
              </Link>
              <Link to="/library/tracks" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="music_note" type="material" />
								Tracks
              </Link>
              <Link to="/library/browse" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="folder" type="material" />
								Browse
              </Link>
            </section>

            <section className="sidebar__menu__section">
              <Link to="/settings" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="settings" type="material" />
								Settings
                {this.renderStatusIcon()}
              </Link>
            </section>

          </nav>
        </div>

        <Dropzones />

        <div className="close" onClick={(e) => uiActions.toggleSidebar(false)}>
          <Icon name="close" />
        </div>

      </aside>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  mopidy_connected: state.mopidy.connected,
  pusher_connected: state.pusher.connected,
  spotify_available: state.spotify.access_token,
  spotify_authorized: state.spotify.authorization,
  snapcast_connected: state.snapcast.connected,
  snapcast_enabled: state.snapcast.enabled,
  update_available: (state.pusher.version && state.pusher.version.update_available ? state.pusher.version.update_available : false),
  test_mode: (state.ui.test_mode ? state.ui.test_mode : false),
  dragger: state.ui.dragger,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(withRouter(Sidebar));

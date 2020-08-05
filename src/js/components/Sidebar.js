
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { withRouter } from 'react-router';
import Link from './Link';
import Icon from './Icon';
import Dropzones from './Fields/Dropzones';
import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import { I18n, i18n } from '../locale';

class Sidebar extends React.Component {

  closeSidebar = () => {
    const {
      uiActions: {
        toggleSidebar,
      },
    } = this.props;

    toggleSidebar(false);
  }

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
          <span className="tooltip__content">
            <I18n path="sidebar.update_available" />
          </span>
        </span>
      );
    }

    if (!navigator.onLine) {
      return (
        <span className="status tooltip tooltip--right">
          <Icon name="wifi_off" className="red-text" />
          <span className="tooltip__content">
            <I18n path="sidebar.browser_offline" />
          </span>
        </span>
      );
    }

    if (!mopidy_connected || !pusher_connected || (!snapcast_connected && snapcast_enabled)) {
      return (
        <span className="status tooltip tooltip--right">
          <Icon name="warning" className="red-text" />
          <span className="tooltip__content">
            {!mopidy_connected && (
              <I18n path="sidebar.not_connected" params={{ name: i18n('services.mopidy.title') }} contentAfter>
                <br />
              </I18n>
            )}
            {!pusher_connected && (
              <I18n path="sidebar.not_connected" params={{ name: i18n('services.pusher.title') }} contentAfter>
                <br />
              </I18n>
            )}
            {!snapcast_connected && snapcast_enabled && (
              <I18n path="sidebar.not_connected" params={{ name: i18n('services.snapcast.title') }} contentAfter>
                <br />
              </I18n>
            )}
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
    } = this.props;

    return (
      <aside className="sidebar">
        <div className="sidebar__liner">
          <nav className="sidebar__menu">

            <section className="sidebar__menu__section">
              <Link to="/queue" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="play_arrow" type="material" />
                <I18n path="sidebar.now_playing" />
              </Link>
              <Link to="/search" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="search" type="material" />
                <I18n path="sidebar.search" />
              </Link>
            </section>

            {spotify_available && (
              <section className="sidebar__menu__section">
                <title className="sidebar__menu__section__title">
                  <I18n path="sidebar.discover" />
                </title>
                <Link to="/discover/recommendations" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                  <Icon name="explore" type="material" />
                  <I18n path="sidebar.discover" />
                </Link>
                <Link to="/discover/categories" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                  <Icon name="mood" type="material" />
                  <I18n path="sidebar.genre" />
                </Link>
                <Link to="/discover/featured" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                  <Icon name="star" type="material" />
                  <I18n path="sidebar.featured_playlists" />
                </Link>
                <Link to="/discover/new-releases" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                  <Icon name="new_releases" type="material" />
                  <I18n path="sidebar.new_releases" />
                </Link>
              </section>
            )}

            <section className="sidebar__menu__section">
              <title className="sidebar__menu__section__title">
                <I18n path="sidebar.my_music" />
              </title>
              <Link to="/library/playlists" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="queue_music" type="material" />
                <I18n path="sidebar.playlists" />
              </Link>
              <Link to="/library/artists" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="recent_actors" type="material" />
                <I18n path="sidebar.artists" />
              </Link>
              <Link to="/library/albums" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="album" type="material" />
                <I18n path="sidebar.albums" />
              </Link>
              <Link to="/library/tracks" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="music_note" type="material" />
                <I18n path="sidebar.tracks" />
              </Link>
              <Link to="/library/browse" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="folder" type="material" />
                <I18n path="sidebar.browse" />
              </Link>
            </section>

            <section className="sidebar__menu__section">
              <Link to="/settings" history={history} className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="settings" type="material" />
                <I18n path="sidebar.settings" />
                {this.renderStatusIcon()}
              </Link>
            </section>

          </nav>
        </div>

        <Dropzones />

        <div className="close" onClick={this.closeSidebar}>
          <Icon name="close" />
        </div>

      </aside>
    );
  }
}

const mapStateToProps = (state) => ({
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

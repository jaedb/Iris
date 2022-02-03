import React from 'react';
import { useSelector, useDispatch } from 'react-redux';
import Link from './Link';
import Icon from './Icon';
import Dropzones from './Fields/Dropzones';
import PinList from './Fields/PinList';
import { I18n, i18n } from '../locale';
import { toggleSidebar } from '../services/ui/actions';

const StatusIcon = () => {
  const update_available = useSelector((state) => state.pusher?.version?.update_available);
  const mopidy_connected = useSelector((state) => state.mopidy.connected);
  const pusher_connected = useSelector((state) => state.pusher.connected);
  const snapcast_connected = useSelector((state) => state.snapcast.connected);
  const snapcast_enabled = useSelector((state) => state.snapcast.enabled);

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
            <I18n path="sidebar.not_connected" name={i18n('services.mopidy.title')} contentAfter>
              <br />
            </I18n>
          )}
          {!pusher_connected && (
            <I18n path="sidebar.not_connected" name={i18n('services.pusher.title')} contentAfter>
              <br />
            </I18n>
          )}
          {!snapcast_connected && snapcast_enabled && (
            <I18n path="sidebar.not_connected" name={i18n('services.snapcast.title')} contentAfter>
              <br />
            </I18n>
          )}
        </span>
      </span>
    );
  }

  return null;
}

const Sidebar = () => {
  const dispatch = useDispatch();
  const spotify_available = useSelector((state) => state.spotify.access_token);

  const close = () => dispatch(toggleSidebar(false));

  return (
    <aside className="sidebar">
      <div className="sidebar__liner">
        <nav className="sidebar__menu">
          <section className="sidebar__menu__section">
            <Link to="/queue" className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
              <Icon name="play_arrow" type="material" />
              <I18n path="sidebar.now_playing" />
            </Link>
            <Link to="/search" className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
              <Icon name="search" type="material" />
              <I18n path="sidebar.search" />
            </Link>
          </section>

          {spotify_available && (
            <section className="sidebar__menu__section">
              <title className="sidebar__menu__section__title">
                <I18n path="sidebar.discover" />
              </title>
              <Link to="/discover/recommendations" className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="explore" type="material" />
                <I18n path="sidebar.discover" />
              </Link>
              <Link to="/discover/categories" className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="mood" type="material" />
                <I18n path="sidebar.genre" />
              </Link>
              <Link to="/discover/featured" className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="star" type="material" />
                <I18n path="sidebar.featured_playlists" />
              </Link>
              <Link to="/discover/new-releases" className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
                <Icon name="new_releases" type="material" />
                <I18n path="sidebar.new_releases" />
              </Link>
            </section>
          )}

          <section className="sidebar__menu__section">
            <title className="sidebar__menu__section__title">
              <I18n path="sidebar.my_music" />
            </title>
            <Link to="/library/playlists" className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
              <Icon name="queue_music" type="material" />
              <I18n path="sidebar.playlists" />
            </Link>
            <PinList />
            <Link to="/library/artists" className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
              <Icon name="recent_actors" type="material" />
              <I18n path="sidebar.artists" />
            </Link>
            <Link to="/library/albums" className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
              <Icon name="album" type="material" />
              <I18n path="sidebar.albums" />
            </Link>
            <Link to="/library/tracks" className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
              <Icon name="music_note" type="material" />
              <I18n path="sidebar.tracks" />
            </Link>
            <Link to="/library/browse" className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
              <Icon name="folder" type="material" />
              <I18n path="sidebar.browse" />
            </Link>
          </section>

          <section className="sidebar__menu__section">
            <Link to="/settings" className="sidebar__menu__item" activeClassName="sidebar__menu__item--active">
              <Icon name="settings" type="material" />
              <I18n path="sidebar.settings" />
              <StatusIcon />
            </Link>
          </section>

        </nav>
      </div>

      <Dropzones />

      <div className="close" onClick={close}>
        <Icon name="close" />
      </div>

    </aside>
  );
};

export default Sidebar;

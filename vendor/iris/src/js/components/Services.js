import React, { useEffect } from 'react';
import { useDispatch } from 'react-redux';
import { Routes, Route } from 'react-router-dom';
import Link from './Link';
import Thumbnail from './Thumbnail';
import Icon from './Icon';
import URILink from './URILink';
import TextField from './Fields/TextField';
import SpotifyAuthenticationFrame from './Fields/SpotifyAuthenticationFrame';
import LastfmAuthenticationFrame from './Fields/LastfmAuthenticationFrame';
import GeniusAuthenticationFrame from './Fields/GeniusAuthenticationFrame';
import Snapcast from './Snapcast';
import * as spotifyActions from '../services/spotify/actions';
import * as lastfmActions from '../services/lastfm/actions';
import * as geniusActions from '../services/genius/actions';
import { I18n } from '../locale';
import Button from './Button';
import { useSelector } from 'react-redux';

const SnapcastMenuItem = () => {
  const { enabled, connected } = useSelector(({ snapcast }) => snapcast);
  return (
    <Link
      className="menu-item menu-item--snapcast"
      activeClassName="menu-item--active"
      to="/settings/services/snapcast"
      scrollTo="#services-menu"
    >
      <div className="menu-item__inner">
        <Icon className="menu-item__icon" name="devices" />
        <div className="menu-item__title">
          <I18n path="services.snapcast.title" />
        </div>
        {!enabled && (
          <span className="status mid_grey-text">
            <I18n path="settings.services.disabled" />
          </span>
        )}
        {enabled && !connected && (
          <span className="status red-text">
            <I18n path="settings.services.disconnected" />
          </span>
        )}
        {enabled && connected && (
          <span className="status green-text">
            <I18n path="settings.services.connected" />
          </span>
        )}
      </div>
    </Link>
  );
}

const SpotifyMenuItem = () => {
  const { enabled, authorization, me } = useSelector(({ spotify }) => spotify);
  const dispatch = useDispatch();

  useEffect(() => {
    if ((enabled || authorization) && !me) {
      dispatch(spotifyActions.getMe());
    }
  }, []);

  return (
    <Link
      className="menu-item menu-item--spotify"
      activeClassName="menu-item--active"
      to="/settings/services/spotify"
      scrollTo="#services-menu"
    >
      <div className="menu-item__inner">
        {me ? (
          <Thumbnail
            className="menu-item__thumbnail"
            circle
            size="small"
            images={me.images}
          />
        ) : (
          <Thumbnail className="menu-item__thumbnail" circle size="small" />
        )}
        <div className="menu-item__title">
          <I18n path="services.spotify.title" />
        </div>
        {authorization ? (
          <span className="status green-text">
            <I18n path="settings.services.authorized" />
          </span>
        ) : (
          <span className="status mid_grey-text">
            <I18n path="settings.services.read_only" />
          </span>
        )}
      </div>
    </Link>
  );
}

const LastfmMenuItem = () => {
  const { authorization, me } = useSelector(({ lastfm }) => lastfm);
  const dispatch = useDispatch();

  useEffect(() => {
    if (authorization) dispatch(lastfmActions.getMe());
  }, []);

  return (
    <Link
      className="menu-item menu-item--lastfm"
      activeClassName="menu-item--active"
      to="/settings/services/lastfm"
      scrollTo="#services-menu"
    >
      <div className="menu-item__inner">
        {me ? (
          <Thumbnail
            className="menu-item__thumbnail"
            circle
            size="small"
            images={me.images}
          />
        ) : (
          <Icon type="fontawesome" name="lastfm" className="menu-item__icon" />
        )}
        <div className="menu-item__title">
          <I18n path="services.lastfm.title" />
        </div>
        {authorization ? (
          <span className="status green-text">
            <I18n path="settings.services.authorized" />
          </span>
        ) : (
          <span className="status mid_grey-text">
            <I18n path="settings.services.read_only" />
          </span>
        )}
      </div>
    </Link>
  );
}

const GeniusMenuItem = () => {
  const { authorization, me } = useSelector(({ genius }) => genius);
  const dispatch = useDispatch();

  useEffect(() => {
    if (authorization && !me) dispatch(geniusActions.getMe());
  }, []);

  return (
    <Link
      className="menu-item menu-item--genius"
      activeClassName="menu-item--active"
      to="/settings/services/genius"
      scrollTo="#services-menu"
    >
      <div className="menu-item__inner">
        {me ? (
          <Thumbnail
            className="menu-item__thumbnail"
            circle
            size="small"
            images={me.images}
          />
        ) : (
          <Icon name="genius" type="svg" className="menu-item__icon" />
        )}
        <div className="menu-item__title">
          <I18n path="services.genius.title" />
        </div>
        {authorization ? (
          <span className="status green-text">
            <I18n path="settings.services.authorized" />
          </span>
        ) : (
          <span className="status mid_grey-text">
            <I18n path="settings.services.unauthorized" />
          </span>
        )}
      </div>
    </Link>
  );
}

const Spotify = () => {
  const { authorization, me, refreshing_token } = useSelector(({ spotify }) => spotify);
  const { locale, country } = useSelector(({ spotify }) => spotify);
  const uri_schemes = useSelector(({ mopidy }) => mopidy.uri_schemes || []);
  const dispatch = useDispatch();
 
  const onChange = (name, value) => dispatch(spotifyActions.set({ [name]: value }))

  return (
    <div className="sub-tabs__content">
      {uri_schemes.indexOf('spotify:') < 0 && (
        <div>
          <p className="message warning">
            <I18n path="settings.services.spotify.mopidy_spotify_not_running" />
          </p>
          <br />
        </div>
      )}
      <label className="field">
        <div className="name">
          <I18n path="settings.services.spotify.country.label" />
        </div>
        <div className="input">
          <TextField
            onChange={(value) => onChange('country', value)}
            value={country}
            autosave
          />
          <div className="description">
            <I18n path="settings.services.spotify.country.description" />
          </div>
        </div>
      </label>
      <label className="field">
        <div className="name">
          <I18n path="settings.services.spotify.locale.title" />
        </div>
        <div className="input">
          <TextField
            onChange={(value) => onChange('locale', value)}
            value={locale}
            autosave
          />
          <div className="description">
            <I18n path="settings.services.spotify.locale.description" />
          </div>
        </div>
      </label>

      <div className="field current-user">
        <div className="name">Current user</div>
        <div className="input">
          <div className="text">
            {me ? (
              <URILink className="user" type="user" uri={me.uri}>
                <Thumbnail circle size="small" images={me.images} />
                <span className="user-name">
                  {me.name || me.id}
                  {!authorization && (
                    <span className="mid_grey-text">
                      {'  '}
                      <I18n path="settings.services.limited_access" />
                    </span>
                  )}
                </span>
              </URILink>
            ) : (
              <URILink className="user">
                <Thumbnail circle size="small" />
                <span className="user-name">
                  <I18n path="settings.services.unknown" />
                </span>
              </URILink>
            )}
          </div>
        </div>
      </div>

      <div className="field">
        <div className="name">
          <I18n path="settings.services.authorization" />
        </div>
        <div className="input">
          <SpotifyAuthenticationFrame />
          <Button
            working={refreshing_token}
            onClick={() => dispatch(spotifyActions.refreshingToken())}
            tracking={{ category: 'Spotify', action: 'RefreshToken' }}
          >
            <I18n path="settings.services.refresh_token" />
          </Button>
        </div>
      </div>
    </div>
  );
}

const Lastfm = () => {
  const { authorization, me } = useSelector(({ lastfm }) => lastfm);

  return (
    <div className="sub-tabs__content">
      {authorization ? (
        <div className="field current-user">
          <div className="name">
            <I18n path="settings.services.current_user" />
          </div>
          <div className="input">
            <div className="text">
              {me ? (
                <span className="user">
                  <Thumbnail circle size="small" images={me.images} />
                  <span className="user-name">
                    {me.name}
                  </span>
                </span>
              ) : (
                <span className="user">
                  <Thumbnail circle size="small" />
                  <span className="user-name">
                    <I18n path="settings.services.unknown" />
                  </span>
                </span>
              )}
            </div>
          </div>
        </div>
      ) : null}

      <div className="field">
        <div className="name">
          <I18n path="settings.services.authorization" />
        </div>
        <div className="input">
          <LastfmAuthenticationFrame />
        </div>
      </div>
    </div>
  );
}

const Genius = () => {
  const { authorization, me } = useSelector(({ genius }) => genius);

  return (
    <div className="sub-tabs__content">
      {authorization && (
        <div className="field current-user">
          <div className="name">
            <I18n path="settings.services.current_user" />
          </div>
          <div className="input">
            <div className="text">
              {me ? (
                <span className="user">
                  <Thumbnail circle size="small" images={me.images} />
                  <span className="user-name">
                    {me.name}
                  </span>
                </span>
              ) : (
                <span className="user">
                  <Thumbnail circle size="small" />
                  <span className="user-name">
                    <I18n path="settings.services.unknown" />
                  </span>
                </span>
              )}
            </div>
          </div>
        </div>
      )}

      <div className="field">
        <div className="name">
          <I18n path="settings.services.authorization" />
        </div>
        <div className="input">
          <GeniusAuthenticationFrame />
        </div>
      </div>
    </div>
  );
}

const Menu = () => (
  <div className="sub-tabs__menu menu" id="services-menu">
    <div className="menu__inner">
      <SnapcastMenuItem />
      <SpotifyMenuItem />
      <LastfmMenuItem />
      <GeniusMenuItem />
    </div>
  </div>
);

const Services = () => (
  <div className="sub-tabs sub-tabs--services">
    <Menu />
    <Routes>
      <Route path="services/spotify" element={<Spotify />} />
      <Route path="services/lastfm" element={<Lastfm />} />
      <Route path="services/genius" element={<Genius />} />
      <Route
        path="services/snapcast/"
        element={(
          <div className="sub-tabs__content">
            <Snapcast />
          </div>
        )}
      />
      <Route
        path="services/snapcast/:groupId"
        element={(
          <div className="sub-tabs__content">
            <Snapcast />
          </div>
        )}
      />
    </Routes>
  </div>
);

export default Services;

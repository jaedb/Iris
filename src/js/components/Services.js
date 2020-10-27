
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Link from './Link';

import Thumbnail from './Thumbnail';
import Icon from './Icon';
import URILink from './URILink';
import TextField from './Fields/TextField';
import SpotifyAuthenticationFrame from './Fields/SpotifyAuthenticationFrame';
import LastfmAuthenticationFrame from './Fields/LastfmAuthenticationFrame';
import GeniusAuthenticationFrame from './Fields/GeniusAuthenticationFrame';
import Snapcast from './Snapcast';

import * as uiActions from '../services/ui/actions';
import * as coreActions from '../services/core/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as pusherActions from '../services/pusher/actions';
import * as spotifyActions from '../services/spotify/actions';
import * as lastfmActions from '../services/lastfm/actions';
import * as geniusActions from '../services/genius/actions';
import { I18n, i18n } from '../locale';
import Button from './Button';

class Services extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      country: this.props.spotify.country,
      locale: this.props.spotify.locale,
      input_in_focus: null,
    };
  }

  componentDidMount() {
    if ((this.props.spotify.enabled || this.props.spotify.authorization) && !this.props.spotify.me) {
      this.props.spotifyActions.getMe();
    }
    if (this.props.lastfm.authorization) {
      this.props.lastfmActions.getMe();
    }
    if (this.props.genius.authorization && !this.props.genius.me) {
      this.props.geniusActions.getMe();
    }
  }

  static getDerivedStateFromProps(props, state) {
    let changed = false;
    const changes = {};

    if (props.spotify.country !== state.country && state.input_in_focus !== 'country') {
      changed = true;
      changes.country = props.spotify.country;
    }
    if (props.spotify.locale !== state.locale && state.input_in_focus !== 'locale') {
      changed = true;
      changes.locale = props.spotify.locale;
    }

    if (changed) return changes;
    return null;
  }

  handleBlur(name, value) {
    this.setState({ input_in_focus: null });
    const data = {};
    data[name] = value;
    this.props.coreActions.set(data);
  }

  renderSpotify = () => {
    const {
      spotify: {
        me,
        authorization,
        refreshing_token,
      },
      mopidy,
      spotifyActions,
    } = this.props;
    const { country, locale } = this.state;
    const user = me ? (
      <URILink className="user" type="user" uri={me.uri}>
        <Thumbnail circle size="small" images={me.images} />
        <span className="user-name">
          {me.name ? me.name : me.id}
          {!authorization && (
            <span className="mid_grey-text">
              {`  (${i18n('settings.services.limited_access')})`}
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
    );

    let not_installed = null;

    if (!mopidy.uri_schemes || !mopidy.uri_schemes.includes('spotify:')) {
      not_installed = (
        <div>
          <p className="message warning">
            <I18n path="settings.services.spotify.mopidy_spotify_not_running" />
          </p>
          <br />
        </div>
      );
    }

    return (
      <div>
        {not_installed}
        <label className="field">
          <div className="name">
            <I18n path="settings.services.spotify.country.label" />
          </div>
          <div className="input">
            <TextField
              onChange={(value) => spotifyActions.set({ country: value })}
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
              onChange={(value) => spotifyActions.set({ locale: value })}
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
              {user}
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
              onClick={() => spotifyActions.refreshingToken()}
              tracking={{ category: 'Spotify', action: 'RefreshToken' }}
            >
              <I18n path="settings.services.refresh_token" />
            </Button>
          </div>
        </div>
      </div>
    );
  }

  renderLastfm = () => {
    const {
      lastfm: {
        me,
        authorization,
      },
    } = this.props;

    const user = me ? (
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
    );

    return (
      <div>
        {authorization ? (
          <div className="field current-user">
            <div className="name">
              <I18n path="settings.services.current_user" />
            </div>
            <div className="input">
              <div className="text">
                {user}
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

  renderGenius = () => {
    const {
      genius: {
        me,
        authorization,
      },
    } = this.props;

    const user = me ? (
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
    );

    return (
      <div>
        {authorization && (
          <div className="field current-user">
            <div className="name">
              <I18n path="settings.services.current_user" />
            </div>
            <div className="input">
              <div className="text">
                {user}
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

  renderMenu = () => {
    const {
      spotify: {
        me: spotifyUser,
        authorization: spotifyAuthorization,
      },
      lastfm: {
        me: lastfmUser,
        authorization: lastfmAuthorization,
      },
      genius: {
        me: geniusUser,
        authorization: geniusAuthorization,
      },
      history,
      snapcast: {
        enabled: snapcastEnabled,
        connected: snapcastConnected,
      }
    } = this.props;

    const spotify_icon = (
      <Thumbnail
        className="menu-item__thumbnail"
        circle
        size="small"
        images={spotifyUser ? spotifyUser.images : null}
      />
    );

    const lastfm_icon = (
      <Thumbnail
        className="menu-item__thumbnail"
        circle
        size="small"
        images={lastfmUser ? lastfmUser.images : null}
      />
    );

    const genius_icon = (
      <Thumbnail
        className="menu-item__thumbnail"
        circle
        size="small"
        images={geniusUser ? geniusUser.images : null}
      />
    );

    return (
      <div className="sub-tabs__menu menu" id="services-menu">
        <div className="menu__inner">
          <Link
            history={history}
            className="menu-item menu-item--spotify"
            activeClassName="menu-item--active"
            to="/settings/services/spotify"
            scrollTo="#services-menu"
          >
            <div className="menu-item__inner">
              {spotify_icon}
              <div className="menu-item__title">
                <I18n path="services.spotify.title" />
              </div>
              {spotifyAuthorization ? (
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
          <Link
            history={history}
            className="menu-item menu-item--lastfm"
            activeClassName="menu-item--active"
            to="/settings/services/lastfm"
            scrollTo="#services-menu"
          >
            <div className="menu-item__inner">
              {lastfm_icon}
              <div className="menu-item__title">
                <I18n path="services.lastfm.title" />
              </div>
              {lastfmAuthorization ? (
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
          <Link
            history={history}
            className="menu-item menu-item--genius"
            activeClassName="menu-item--active"
            to="/settings/services/genius"
            scrollTo="#services-menu"
          >
            <div className="menu-item__inner">
              {genius_icon}
              <div className="menu-item__title">
                <I18n path="services.genius.title" />
              </div>
              {geniusAuthorization ? (
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
          <Link
            history={history}
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
              {!snapcastEnabled && (
                <span className="status mid_grey-text">
                  <I18n path="settings.services.disabled" />
                </span>
              )}
              {snapcastEnabled && !snapcastConnected && (
                <span className="status red-text">
                  <I18n path="settings.services.disconnected" />
                </span>
              )}
              {snapcastEnabled && snapcastConnected && (
                <span className="status green-text">
                  <I18n path="settings.services.connected" />
                </span>
              )}
            </div>
          </Link>
        </div>
      </div>
    );
  }

  renderService() {
    const { match } = this.props;
    switch (match.params.service) {
      case 'spotify':
        return <div className="sub-tabs__content">{this.renderSpotify()}</div>;
      case 'lastfm':
        return <div className="sub-tabs__content">{this.renderLastfm()}</div>;
      case 'genius':
        return <div className="sub-tabs__content">{this.renderGenius()}</div>;
      case 'snapcast':
        return <div className="sub-tabs__content">{<Snapcast match={this.props.match} />}</div>;
      default:
        return null;
    }
  }

  render() {
    return (
      <div className="sub-tabs sub-tabs--services">
        {this.renderMenu()}
        {this.renderService()}
      </div>
    );
  }
}

const mapStateToProps = (state) => state;

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  pusherActions: bindActionCreators(pusherActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
  geniusActions: bindActionCreators(geniusActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Services);

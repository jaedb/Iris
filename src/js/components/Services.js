
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
import { I18n } from '../locale';
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
    if ((this.props.spotify.enabled || this.props.spotify.authorization) && (!this.props.spotify.me || this.props.core.users[this.props.spotify.me.id] === undefined)) {
      this.props.spotifyActions.getMe();
    }
    if (this.props.lastfm.authorization && this.props.core.users[`lastfm:user:${this.props.lastfm.authorization.name}`] === undefined) {
      this.props.lastfmActions.getMe();
    }
    if (this.props.genius.authorization && (!this.props.genius.me || this.props.core.users[`genius:user:${this.props.genius.me.id}`] === undefined)) {
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

  renderSpotify() {
    const {
      core,
      spotify,
      mopidy,
      spotifyActions,
    } = this.props;
    const { country, locale } = this.state;
    const user_object = (spotify.me && core.users[spotify.me.uri] ? core.users[spotify.me.uri] : null);
    if (user_object) {
      var user = (
        <URILink className="user" type="user" uri={user_object.uri}>
          <Thumbnail circle size="small" images={user_object.images} />
          <span className="user-name">
            {user_object.name ? user_object.name : user_object.id}
            {!this.props.spotify.authorization && (
              <span className="mid_grey-text">
                {'  ('}
                <I18n path="settings.services.limited_access" />
                {')'}
              </span>
            )}
          </span>
        </URILink>
      );
    } else {
      var user = (
        <URILink className="user">
          <Thumbnail circle size="small" />
          <span className="user-name">
						<I18n path="settings.services.unknown" />
          </span>
        </URILink>
      );
    }

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
              working={spotify.refreshing_token}
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

  renderLastfm() {
    const user_object = (this.props.lastfm.me ? this.props.core.users[`lastfm:user:${this.props.lastfm.me.name}`] : null);
    if (user_object) {
      var user = (
        <span className="user">
          <Thumbnail circle size="small" images={user_object.images} />
          <span className="user-name">
            {user_object.name}
          </span>
        </span>
      );
    } else {
      var user = (
        <span className="user">
          <Thumbnail circle size="small" />
          <span className="user-name">
            <I18n path="settings.services.unknown" />
          </span>
        </span>
      );
    }

    return (
      <div>
        {this.props.lastfm.authorization ? (
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

  renderGenius() {
    const user_object = (this.props.genius.me ? this.props.core.users[this.props.genius.me.uri] : null);
    if (user_object) {
      var user = (
        <span className="user">
          <Thumbnail circle size="small" images={user_object.images} />
          <span className="user-name">
            {user_object.name}
          </span>
        </span>
      );
    } else {
      var user = (
        <span className="user">
          <Thumbnail circle size="small" />
          <span className="user-name">
            <I18n path="settings.services.unknown" />
          </span>
        </span>
      );
    }

    return (
      <div>
        {this.props.genius.authorization ? (
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
            <GeniusAuthenticationFrame />
          </div>
        </div>
      </div>
    );
  }

  renderIcecast() {
    const { core, coreActions } = this.props;
    return (
      <div>
        <div className="field checkbox">
          <div className="name">
            <I18n path="settings.services.icecast.enable.label" />
          </div>
          <div className="input">
            <label>
              <input
                type="checkbox"
                name="ssl"
                checked={core.http_streaming_enabled}
                onChange={() => coreActions.set({ http_streaming_enabled: !core.http_streaming_enabled })}
              />
              <span className="label">
                <I18n path="settings.services.icecast.enable.description" />
              </span>
            </label>
          </div>
        </div>
        <label className="field">
          <div className="name">
            <I18n path="settings.services.icecast.location.label" />
          </div>
          <div className="input">
            <TextField
              onChange={(value) => coreActions.set({ http_streaming_url: value })}
              value={core.http_streaming_url}
              autosave
            />
            <div className="description">
              <I18n path="settings.services.icecast.location.description" />
            </div>
          </div>
        </label>
      </div>
    );
  }

  renderMenu() {
    if (this.props.spotify.me && this.props.core.users[this.props.spotify.me.uri]) {
      var spotify_icon = <Thumbnail className="menu-item__thumbnail" circle size="small" images={this.props.core.users[this.props.spotify.me.uri].images} />;
    } else {
      var spotify_icon = <Thumbnail className="menu-item__thumbnail" circle size="small" />;
    }

    if (this.props.lastfm.me && this.props.core.users[this.props.lastfm.me.uri]) {
      var lastfm_icon = <Thumbnail className="menu-item__thumbnail" circle size="small" images={this.props.core.users[this.props.lastfm.me.uri].images} />;
    } else {
      var lastfm_icon = <Icon type="fontawesome" name="lastfm" className="menu-item__icon" />;
    }

    if (this.props.genius.me && this.props.core.users[this.props.genius.me.uri]) {
      var genius_icon = <Thumbnail className="menu-item__thumbnail" circle size="small" images={this.props.core.users[this.props.genius.me.uri].images} />;
    } else {
      var genius_icon = <Icon name="genius" type="svg" className="menu-item__icon" />;
    }

    return (
      <div className="sub-tabs__menu menu" id="services-menu">
        <div className="menu__inner">
          <Link history={this.props.history} className="menu-item menu-item--spotify" activeClassName="menu-item--active" to="/settings/services/spotify" scrollTo="#services-menu">
            <div className="menu-item__inner">
              {spotify_icon}
              <div className="menu-item__title">
                <I18n path="services.spotify.title" />
              </div>
              {this.props.spotify.authorization ? (
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
          <Link history={this.props.history} className="menu-item menu-item--lastfm" activeClassName="menu-item--active" to="/settings/services/lastfm" scrollTo="#services-menu">
            <div className="menu-item__inner">
              {lastfm_icon}
              <div className="menu-item__title">
                <I18n path="services.lastfm.title" />
              </div>
              {this.props.lastfm.authorization ? (
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
          <Link history={this.props.history} className="menu-item menu-item--genius" activeClassName="menu-item--active" to="/settings/services/genius" scrollTo="#services-menu">
            <div className="menu-item__inner">
              {genius_icon}
              <div className="menu-item__title">
                <I18n path="services.genius.title" />
              </div>
              {this.props.genius.authorization ? (
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
          <Link history={this.props.history} className="menu-item menu-item--snapcast" activeClassName="menu-item--active" to="/settings/services/snapcast" scrollTo="#services-menu">
            <div className="menu-item__inner">
              <Icon className="menu-item__icon" name="devices" />
              <div className="menu-item__title">
                <I18n path="services.snapcast.title" />
              </div>
              {!this.props.snapcast.enabled && (
                <span className="status mid_grey-text">
                  <I18n path="settings.services.disabled" />
                </span>
              )}
              {this.props.snapcast.enabled && !this.props.snapcast.connected && (
                <span className="status red-text">
                  <I18n path="settings.services.disconnected" />
                </span>
              )}
              {this.props.snapcast.enabled && this.props.snapcast.connected && (
                <span className="status green-text">
                  <I18n path="settings.services.connected" />
                </span>
              )}
            </div>
          </Link>
          <Link history={this.props.history} className="menu-item menu-item--icecast" activeClassName="menu-item--active" to="/settings/services/icecast" scrollTo="#services-menu">
            <div className="menu-item__inner">
              <Icon className="menu-item__icon" name="wifi_tethering" />
              <div className="menu-item__title">
                <I18n path="services.icecast.title" />
              </div>
              {this.props.core.http_streaming_enabled ? (
                <span className="status green-text">
                  <I18n path="settings.services.enabled" />
                </span>
              ) : (
                <span className="status mid_grey-text">
                  <I18n path="settings.services.disabled" />
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
      case 'icecast':
        return <div className="sub-tabs__content">{this.renderIcecast()}</div>;
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

const mapStateToProps = (state, ownProps) => state;

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

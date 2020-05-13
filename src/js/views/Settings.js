
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { Route, Link } from 'react-router-dom';
import ConfirmationButton from '../components/Fields/ConfirmationButton';
import PusherConnectionList from '../components/PusherConnectionList';
import SourcesPriority from '../components/Fields/SourcesPriority';
import Commands from '../components/Fields/Commands';
import TextField from '../components/Fields/TextField';
import Header from '../components/Header';
import Icon from '../components/Icon';
import Services from '../components/Services';
import Servers from '../components/Servers';
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as pusherActions from '../services/pusher/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as lastfmActions from '../services/lastfm/actions';
import * as spotifyActions from '../services/spotify/actions';
import { isHosted } from '../util/helpers';

class Settings extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      mopidy_host: this.props.mopidy.host,
      mopidy_port: this.props.mopidy.port,
      mopidy_library_artists_uri: this.props.mopidy.library_artists_uri,
      mopidy_library_albums_uri: this.props.mopidy.library_albums_uri,
      pusher_username: this.props.pusher.username,
      input_in_focus: null,
    };
  }

  componentDidMount() {
    this.props.uiActions.setWindowTitle('Settings');
  }

  componentDidUpdate = () => {
    const { pusher: { username } } = this.props;
    const { pusher_username, input_in_focus } = this.state;
    let new_username = null;

    if (username && username !== pusher_username && input_in_focus !== 'pusher_username') {
      new_username = username;
    }

    if (new_username) this.setState({ pusher_username: new_username });
  }

  resetAllSettings() {
    localStorage.clear();
    window.location = '#';
    window.location.reload(true);
    return false;
  }

  resetServiceWorkerAndCache() {
    if ('serviceWorker' in navigator) {

      // Hose out all our caches
      caches.keys().then(function (cacheNames) {
        cacheNames.forEach(function (cacheName) {
          caches.delete(cacheName);
        });
      });

      // Unregister all service workers
      // This forces our SW to bugger off and a new one is registered on refresh
      navigator.serviceWorker.getRegistrations().then(
        (registrations) => {
          for (let registration of registrations) {
            registration.unregister();
          }
        }
      );

      window.location = '#';
      window.location.reload(true);
    } else {
      this.props.coreActions.handleException('Service Worker not supported');
    }
  }

  handleBlur(service, name, value) {
    this.setState({ input_in_focus: null });
    const data = {};
    data[name] = value;
    this.props[`${service}Actions`].set(data);

    // Any per-field actions
    switch (name) {
      case 'library_albums_uri':
        this.props.mopidyActions.clearLibraryAlbums();
        break;
      case 'library_artists_uri':
        this.props.mopidyActions.clearLibraryArtists();
        break;
    }
  }

  renderLocalScanButton = () => {
    const { processes } = this.props.ui;
    const loading = processes.local_scan && processes.local_scan.status === 'running';
    return (
      <button
        className={`button button--default ${loading ? 'button--working' : ''}`}
        onClick={(e) => this.props.pusherActions.localScan()}
      >
        Run local scan
      </button>
    );
  }

  render() {
    const {
      mopidyActions,
      mopidy,
      pusherActions,
      pusher,
      history,
      uiActions,
      ui,
    } = this.props;

    const options = (
      <span>
        <a className="button button--discrete button--no-hover" onClick={(e) => history.push('/settings/debug')}>
          <Icon name="code" />
          Debug
        </a>
        <a className="button button--discrete button--no-hover" href="https://github.com/jaedb/Iris/wiki" target="_blank">
          <Icon name="help" />
          Help
        </a>
      </span>
    );

    return (
      <div className="view settings-view">
        <Header options={options} uiActions={uiActions}>
          <Icon name="settings" type="material" />
          Settings
        </Header>

        <section className="content-wrapper">

          <h4 className="underline">
            Server
            <a name="server" />
          </h4>

          <label className="field">
            <div className="name">Username</div>
            <div className="input">
              <TextField
                onChange={(value) => pusherActions.setUsername(value.replace(/\W/g, ''))}
                value={this.state.pusher_username}
                autosave
              />
              <div className="description">
                A non-unique string used to identify this client (no special characters)
              </div>
            </div>
          </label>

          <Route path="/settings/:server?/:id?" component={Servers} />

          <h4 className="underline">
            Services
            <a name="services" />
          </h4>

          <Route path="/settings/:services?/:service?/:id?" component={Services} />

          <h4 className="underline">
            Interface
            <a name="interface" />
          </h4>

          <div className="field radio">
            <div className="name">
              Theme
            </div>
            <div className="input">
              <label>
                <input
                  type="radio"
                  name="theme"
                  value="auto"
                  checked={ui.theme === 'auto'}
                  onChange={(e) => uiActions.set({ theme: e.target.value })}
                />
                <span className="label tooltip">
                  Auto
                  <span className="tooltip__content">Detects your browser or OS preference</span>
                </span>
              </label>
              <label>
                <input
                  type="radio"
                  name="theme"
                  value="dark"
                  checked={ui.theme === 'dark'}
                  onChange={(e) => uiActions.set({ theme: e.target.value })}
                />
                <span className="label">Dark</span>
              </label>
              <label>
                <input
                  type="radio"
                  name="theme"
                  value="light"
                  checked={ui.theme === 'light'}
                  onChange={(e) => uiActions.set({ theme: e.target.value })}
                />
                <span className="label">Light</span>
              </label>
            </div>
          </div>

          <div className="field checkbox">
            <div className="name">Behavior</div>
            <div className="input">
              <label>
                <input
                  type="checkbox"
                  name="log_actions"
                  checked={ui.clear_tracklist_on_play}
                  onChange={(e) => uiActions.set({ clear_tracklist_on_play: !ui.clear_tracklist_on_play })}
                />
                <span className="label tooltip">
                  Clear tracklist on play of URI(s)
                  <span className="tooltip__content">Playing one or more URIs will clear the current play queue first</span>
                </span>
              </label>
              <label>
                <input
                  type="checkbox"
                  name="hotkeys_enabled"
                  checked={ui.hotkeys_enabled}
                  onChange={(e) => uiActions.set({ hotkeys_enabled: !ui.hotkeys_enabled })}
                />
                <span className="label">
                  Enable hotkeys
                </span>
              </label>
              <label>
                <input
                  type="checkbox"
                  name="smooth_scrolling_enabled"
                  checked={ui.smooth_scrolling_enabled}
                  onChange={(e) => uiActions.set({ smooth_scrolling_enabled: !ui.smooth_scrolling_enabled })}
                />
                <span className="label">
                  Enable smooth scrolling
                </span>
              </label>
              <label>
                <input
                  type="checkbox"
                  name="playback_controls_touch_enabled"
                  checked={ui.playback_controls_touch_enabled}
                  onChange={(e) => uiActions.set({ playback_controls_touch_enabled: !ui.playback_controls_touch_enabled })}
                />
                <span className="label tooltip">
                  Enable touch events on play controls
                  <span className="tooltip__content">Allows left- and right-swipe to change tracks</span>
                </span>
              </label>
              <label>
                <input
                  type="checkbox"
                  name="wide_scrollbar_enabled"
                  checked={ui.wide_scrollbar_enabled}
                  onChange={(e) => uiActions.set({ wide_scrollbar_enabled: !ui.wide_scrollbar_enabled })}
                />
                <span className="label">
                  Use wide scrollbars
                </span>
              </label>
            </div>
          </div>

          <div className="field sources-priority">
            <div className="name">
              Sources priority
            </div>
            <div className="input">
              <SourcesPriority
                uri_schemes={mopidy.uri_schemes ? mopidy.uri_schemes : []}
                uri_schemes_priority={ui.uri_schemes_priority ? ui.uri_schemes_priority : []}
                uiActions={uiActions}
              />
              <div className="description">
                Drag-and-drop to prioritize search providers and results
              </div>
            </div>
          </div>

          {isHosted() ? null : (
            <div className="field checkbox">
              <div className="name">Reporting</div>
              <div className="input">
                <label>
                  <input
                    type="checkbox"
                    name="allow_reporting"
                    checked={ui.allow_reporting}
                    onChange={(e) => uiActions.set({ allow_reporting: !ui.allow_reporting })}
                  />
                  <span className="label">
                    Allow reporting of anonymous usage statistics
                  </span>
                </label>
                <div className="description">
                  This helps identify errors and potential features that make Iris better for everyone. See
                  <a href="https://github.com/jaedb/Iris/wiki/Terms-of-use#privacy-policy" target="_blank">privacy policy</a>
                  .
                </div>
              </div>
            </div>
          )}

          <div className="field commands-setup" id="commands-setup">
            <div className="name">
              Commands
            </div>
            <div className="input">
              <Commands
                commands={this.props.pusher.commands}
                runCommand={(id, notify) => this.props.pusherActions.runCommand(id, notify)}
                onChange={(commands) => this.props.pusherActions.setCommands(commands)}
              />
              <Link to="/edit-command" className="button button--default">Add new</Link>
            </div>
          </div>

          <h4 className="underline">
            Advanced
            <a name="advanced" />
          </h4>

          <div className="field">
            <div className="name">Artist library URI</div>
            <div className="input">
              <TextField
                value={this.state.mopidy_library_artists_uri}
                onChange={(mopidy_library_artists_uri) => this.setState({ mopidy_library_artists_uri })}
                autosave
              />
              <div className="description">
                URI used for collecting library artists
              </div>
            </div>
          </div>

          <label className="field">
            <div className="name">Album library URI</div>
            <div className="input">
              <TextField
                type="text"
                value={this.state.mopidy_library_albums_uri}
                onChange={(mopidy_library_albums_uri) => this.setState({ mopidy_library_albums_uri })}
                autosave
              />
              <div className="description">
                URI used for collecting library albums
              </div>
            </div>
          </label>

          <div className="field pusher-connections">
            <div className="name">Connections</div>
            <div className="input">
              <div className="text">
                <PusherConnectionList />
              </div>
            </div>
          </div>

          <div className="field">
            <div className="name">Version</div>
            <div className="input">
              <span className="text">
                {this.props.pusher.version.current}
                {' '}
                <span className="mid_grey-text">
                  {build}
                </span>
                {this.props.pusher.version.upgrade_available ? (
                  <span className="flag flag--dark">
                    <Icon name="cloud_download" className="blue-text" />
                    {'  Upgrade available'}
                  </span>
                ) : (
                    <span className="flag flag--dark">
                      <Icon name="check" className="green-text" />
                      {'  Up-to-date'}
                    </span>
                )}
              </span>
            </div>
          </div>

          <div className="field">
            {this.renderLocalScanButton()}
            <Link className="button button--default" to="/share-configuration">Share configuration</Link>
          </div>

          <div className="field">
            {this.props.pusher.version.upgrade_available ? (
              <button className="button button--secondary" onClick={(e) => this.props.pusherActions.upgrade()}>
                {`Upgrade to ${this.props.pusher.version.latest}`}
              </button>
            ) : null}
            <button className={`button button--destructive${this.props.mopidy.restarting ? ' button--working' : ''}`} onClick={(e) => this.props.pusherActions.restart()}>Restart server</button>
            <ConfirmationButton
              className="button--destructive"
              content="Reset all settings"
              confirmingContent="Are you sure?"
              onConfirm={() => this.resetAllSettings()}
            />
            <button
              className="button button--destructive"
              onClick={() => this.resetServiceWorkerAndCache()}
            >
              Reset cache
            </button>
          </div>

          <h4 className="underline">
            About
            <a name="about" />
          </h4>

          <div>
            <em><a href="https://github.com/jaedb/Iris" target="_blank">Iris</a></em>
            {' '}
            is an open-source project by
            <a href="https://github.com/jaedb" target="_blank">James Barnsley</a>
            . It is provided free and with absolutely no warranty. If you paid someone for this software, please let me know.
          </div>
          <br />
          <br />
          <div>
            <a className="button button--default" href="https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=james%40barnsley%2enz&lc=NZ&item_name=James%20Barnsley&currency_code=USD&bn=PP%2dDonationsBF%3abtn_donate_LG%2egif%3aNonHosted" target="_blank">
              <Icon type="fontawesome" name="paypal" />
              {' '}
              Donate
            </a>
            <a className="button button--default" href="https://github.com/jaedb/Iris" target="_blank">
              <Icon type="fontawesome" name="github" />
              {' '}
              GitHub
            </a>
          </div>

        </section>
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
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Settings);

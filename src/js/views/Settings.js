import React from 'react';
import { connect, useDispatch } from 'react-redux';
import { bindActionCreators } from 'redux';
import { Routes, Route } from 'react-router-dom';
import PusherConnectionList from '../components/PusherConnectionList';
import SourcesPriority from '../components/Fields/SourcesPriority';
import Commands from '../components/Fields/Commands';
import TextField from '../components/Fields/TextField';
import SelectField from '../components/Fields/SelectField';
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
import { i18n, I18n, languagesAvailable } from '../locale';
import Button from '../components/Button';
import { dater } from '../components/Dater';

const CheckboxSetting = ({
  name,
  checked,
  label,
  tooltip,
}) => {
  const dispatch = useDispatch();
  return (
    <label>
      <input
        type="checkbox"
        name={name}
        checked={checked}
        onChange={() => dispatch(uiActions.set({ [name]: !checked }))}
      />
      <span className={`label ${tooltip ? 'tooltip' : ''}`}>
        {label}
        {tooltip && (
          <span className="tooltip__content">
            {tooltip}
          </span>
        )}
      </span>
    </label>
  );
};

class Settings extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      mopidy_host: this.props.mopidy.host,
      mopidy_port: this.props.mopidy.port,
      mopidy_library_artists_uri: this.props.mopidy.library_artists_uri,
      mopidy_library_albums_uri: this.props.mopidy.library_albums_uri,
      mopidy_library_tracks_uri: this.props.mopidy.library_tracks_uri,
      pusher_username: this.props.pusher.username,
      input_in_focus: null,
    };
  }

  componentDidMount() {
    const { uiActions: { setWindowTitle } } = this.props;
    setWindowTitle(i18n('settings.title'));
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

  onLanguageChange = (language) => {
    const { uiActions: { setLanguage } } = this.props;
    setLanguage(language);
  }

  onMopidySettingChanged = (name, value) => {
    const {
      mopidyActions: {
        set,
      },
    } = this.props;
    const shortName = name.replace('mopidy_', '');
    set({ [shortName]: value });
    this.setState({ [name]: value });
  }

  doRestart = () => {
    const { pusherActions: { restart } } = this.props;
    restart();
  }

  doUpgrade = () => {
    const { pusherActions: { upgrade } } = this.props;
    upgrade();
  }

  doLocalScan = () => {
    const { pusherActions: { localScan } } = this.props;
    localScan();
  }

  renderLocalScanButton = () => {
    const {
      ui: { processes },
    } = this.props;

    const loading = processes.local_scan && processes.local_scan.status === 'running';

    return (
      <Button
        working={loading}
        onClick={this.doLocalScan}
        tracking={{ category: 'System', action: 'LocalScan' }}
      >
        <I18n path="settings.advanced.start_local_scan" />
      </Button>
    );
  }

  render = () => {
    const {
      mopidyActions: {
        set: setMopidy,
      },
      mopidy,
      pusherActions: {
        setUsername,
        runCommand,
        setCommands,
      },
      pusher,
      history,
      uiActions,
      ui,
    } = this.props;
    const {
      pusher_username,
    } = this.state;

    const options = (
      <>
        <Button
          discrete
          noHover
          to="/settings/debug"
        >
          <I18n path="debug.title">
            <Icon name="code" />
          </I18n>
        </Button>
        <Button
          href="https://github.com/jaedb/Iris/wiki"
          target="_blank"
          rel="noreferrer noopener"
          discrete
          noHover
        >
          <I18n path="settings.help">
            <Icon name="help" />
          </I18n>
        </Button>
      </>
    );

    return (
      <div className="view settings-view">
        <Header options={options} uiActions={uiActions}>
          <I18n path="settings.title">
            <Icon name="settings" type="material" />
          </I18n>
        </Header>

        <section className="content-wrapper">

          <h4 className="underline">
            <I18n path="settings.server.title" />
            <a name="server" />
          </h4>

          <label className="field">
            <div className="name"><I18n path="settings.server.username.label" /></div>
            <div className="input">
              <TextField
                onChange={(value) => setUsername(value.replace(/\W/g, ''))}
                value={pusher_username}
                autosave
              />
              <div className="description">
                <I18n path="settings.server.username.description" />
              </div>
            </div>
          </label>

          <Routes>
            <Route path=":server/*" element={<Servers />} />
          </Routes>
          <h4 className="underline">
            <I18n path="settings.services.title" />
            <a name="services" />
          </h4>
          <Routes>
            <Route path="" element={<Services />} />
            <Route path=":services/" element={<Services />} />
            <Route path=":services/:service/" element={<Services />} />
            <Route path=":services/:service/:id" element={<Services />} />
          </Routes>

          <h4 className="underline">
            <I18n path="settings.interface.title" />
            <a name="interface" />
          </h4>

          <div className="field dropdown">
            <div className="name"><I18n path="settings.interface.language.label" /></div>
            <div className="input">
              <SelectField
                onChange={this.onLanguageChange}
                options={languagesAvailable.map((language) => ({
                  value: language.key,
                  label: `${language.name} (${language.key})`,
                }))}
                value={ui.language}
              />
            </div>
          </div>

          <div className="field radio">
            <div className="name">
              <I18n path="settings.interface.theme.label" />
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
                  <I18n path="settings.interface.theme.auto" />
                  <span className="tooltip__content">
                    <I18n path="settings.interface.theme.auto_tooltip" />
                  </span>
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
                <span className="label">
                  <I18n path="settings.interface.theme.dark" />
                </span>
              </label>
              <label>
                <input
                  type="radio"
                  name="theme"
                  value="light"
                  checked={ui.theme === 'light'}
                  onChange={(e) => uiActions.set({ theme: e.target.value })}
                />
                <span className="label">
                  <I18n path="settings.interface.theme.light" />
                </span>
              </label>
            </div>
          </div>

          <div className="field checkbox">
            <div className="name"><I18n path="settings.interface.behavior.label" /></div>
            <div className="input">
              <CheckboxSetting
                name="clear_tracklist_on_play"
                checked={ui.clear_tracklist_on_play}
                label={i18n('settings.interface.behavior.clear_tracklist')}
                tooltip={i18n('settings.interface.behavior.clear_tracklist_tooltip')}
              />
              <CheckboxSetting
                name="hotkeys_enabled"
                checked={ui.hotkeys_enabled}
                label={i18n('settings.interface.behavior.hotkeys')}
              />
              <CheckboxSetting
                name="playback_controls_touch_enabled"
                checked={ui.playback_controls_touch_enabled}
                label={i18n('settings.interface.behavior.touch_events')}
                tooltip={i18n('settings.interface.behavior.touch_events_tooltip')}
              />
              <CheckboxSetting
                name="grid_glow_enabled"
                checked={ui.grid_glow_enabled}
                label={i18n('settings.interface.behavior.grid_glow')}
                tooltip={i18n('settings.interface.behavior.grid_glow_tooltip')}
              />
            </div>
          </div>

          <div className="field checkbox">
            <div className="name"><I18n path="settings.interface.scrolling.label" /></div>
            <div className="input">
              <CheckboxSetting
                name="smooth_scrolling_enabled"
                checked={ui.smooth_scrolling_enabled}
                label={i18n('settings.interface.scrolling.smooth_scrolling')}
              />
              <CheckboxSetting
                name="wide_scrollbar_enabled"
                checked={ui.wide_scrollbar_enabled}
                label={i18n('settings.interface.scrolling.wide')}
              />
              <CheckboxSetting
                name="hide_scrollbars"
                checked={ui.hide_scrollbars}
                label={i18n('settings.interface.scrolling.hidden')}
              />
            </div>
          </div>

          <div className="field sources-priority">
            <div className="name">
              <I18n path="settings.interface.sources_priority.label" />
            </div>
            <div className="input">
              <SourcesPriority
                uri_schemes={mopidy.uri_schemes ? mopidy.uri_schemes : []}
                uri_schemes_priority={ui.uri_schemes_priority ? ui.uri_schemes_priority : []}
                uiActions={uiActions}
              />
              <div className="description">
                <I18n path="settings.interface.sources_priority.description" />
              </div>
            </div>
          </div>

          {isHosted() ? null : (
            <div className="field checkbox">
              <div className="name">
                <I18n path="settings.interface.reporting.label" />
              </div>
              <div className="input">
                <CheckboxSetting
                  name="allow_reporting"
                  checked={ui.allow_reporting}
                  label={i18n('settings.interface.reporting.sublabel')}
                />
                <div className="description">
                  <I18n path="settings.interface.reporting.description" />
                  <a href="https://github.com/jaedb/Iris/wiki/Terms-of-use#privacy-policy" target="_blank"><I18n path="settings.interface.reporting.privacy_policy" /></a>
                  .
                </div>
              </div>
            </div>
          )}

          <div className="field commands-setup" id="commands-setup">
            <div className="name">
              <I18n path="settings.interface.commands.label" />
            </div>
            <div className="input">
              <Commands
                commands={pusher.commands}
                runCommand={(id, notify) => runCommand(id, notify)}
                onChange={(commands) => setCommands(commands)}
              />
              <Button to="/modal/edit-command">
                <I18n path="actions.add" />
              </Button>
            </div>
          </div>

          <h4 className="underline">
            <I18n path="settings.advanced.title" />
            <a name="advanced" />
          </h4>

          <div className="field">
            <div className="name">
              <I18n path="settings.advanced.artist_uri.label" />
            </div>
            <div className="input">
              <TextField
                value={this.state.mopidy_library_artists_uri}
                onChange={(value) => this.onMopidySettingChanged('mopidy_library_artists_uri', value)}
                autosave
              />
              <div className="description">
                <I18n path="settings.advanced.artist_uri.description" />
              </div>
            </div>
          </div>

          <label className="field">
            <div className="name">
              <I18n path="settings.advanced.album_uri.label" />
            </div>
            <div className="input">
              <TextField
                type="text"
                value={this.state.mopidy_library_albums_uri}
                onChange={(value) => this.onMopidySettingChanged('mopidy_library_albums_uri', value)}
                autosave
              />
              <div className="description">
                <I18n path="settings.advanced.album_uri.description" />
              </div>
            </div>
          </label>

          <label className="field">
            <div className="name">
              <I18n path="settings.advanced.track_uri.label" />
            </div>
            <div className="input">
              <TextField
                type="text"
                value={this.state.mopidy_library_tracks_uri}
                onChange={(value) => this.onMopidySettingChanged('mopidy_library_tracks_uri', value)}
                autosave
              />
              <div className="description">
                <I18n path="settings.advanced.track_uri.description" />
              </div>
            </div>
          </label>

          <div className="field pusher-connections">
            <div className="name">
              <I18n path="settings.advanced.connections.label" />
            </div>
            <div className="input">
              <div className="text">
                <PusherConnectionList />
              </div>
            </div>
          </div>

          <div className="field">
            <div className="name">
              <I18n path="settings.advanced.version.label" />
            </div>
            <div className="input">
              <span className="text">
                <a
                  target="_blank"
                  rel="noopener noreferrer"
                  href={`https://github.com/jaedb/Iris/releases/tag/${pusher.version.current}`}
                >
                  {pusher.version.current}
                </a>
                {' '}
                <span className="mid_grey-text tooltip">
                  {window.build}
                  {window.build && (
                    <span className="tooltip__content">
                      {`${dater('ago', parseInt(window.build, 10) * 1000)} ago`}
                    </span>
                  )}
                </span>
                {pusher.version.upgrade_available ? (
                  <>
                    <a
                      target="_blank"
                      rel="noopener noreferrer"
                      className="flag flag--dark"
                      href={`https://github.com/jaedb/Iris/releases/tag/${pusher.version.latest}`}
                    >
                      <Icon name="cloud_download" className="blue-text" />
                      <I18n
                        path="settings.advanced.version.upgrade_available"
                        version={pusher.version.latest}
                      >
                        {' '}
                      </I18n>
                    </a>
                    <Button
                      type="secondary"
                      onClick={this.doUpgrade}
                      tracking={{ category: 'System', action: 'Upgrade', label: pusher.version.latest }}
                    >
                      <I18n path="settings.advanced.version.upgrade" version={pusher.version.latest} />
                    </Button>
                  </>
                ) : (
                  <span className="flag flag--dark">
                    <Icon name="check" className="green-text" />
                    <I18n path="settings.advanced.version.up_to_date">
                      {' '}
                    </I18n>
                  </span>
                )}
              </span>
            </div>
          </div>

          <div className="field">
            <div className="name">
              <I18n path="settings.advanced.shared_configuration.label" />
            </div>
            <div className="input">
              <Button to="/modal/share-config">
                <I18n path="settings.advanced.shared_configuration.share" />
              </Button>
              {pusher.shared_config && (
                <Button to="/modal/import-config/server">
                  <I18n path="settings.advanced.shared_configuration.import" />
                </Button>
              )}
            </div>
          </div>

          <div className="field">
            {this.renderLocalScanButton()}
            <Button
              type="destructive"
              working={mopidy.restarting}
              onClick={this.doRestart}
              tracking={{ category: 'System', action: 'Restart' }}
            >
              <I18n path="settings.advanced.restart" />
            </Button>
            <Button
              to="/modal/reset"
              type="destructive"
            >
              <I18n path="settings.advanced.reset" />
            </Button>
          </div>

          <h4 className="underline">
            <I18n path="settings.about.title" />
            <a name="about" />
          </h4>

          <div>
            <em><a href="https://github.com/jaedb/Iris" target="_blank">Iris</a></em>
            <I18n path="settings.about.blurb_1" />
            <a
              href="https://github.com/jaedb"
              target="_blank"
              rel="noopener noreferrer"
            >
              James Barnsley
            </a>
            <I18n path="settings.about.blurb_2" />
          </div>
          <br />
          <br />
          <div>
            <Button
              href="https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=james%40barnsley%2enz&lc=NZ&item_name=James%20Barnsley&currency_code=USD&bn=PP%2dDonationsBF%3abtn_donate_LG%2egif%3aNonHosted"
              target="_blank"
              tracking={{ category: 'About', action: 'Paypal' }}
            >
              <I18n path="settings.about.donate">
                <Icon type="fontawesome" name="paypal" />
                {' '}
              </I18n>
            </Button>
            <Button
              href="https://github.com/jaedb/Iris"
              target="_blank"
              tracking={{ category: 'About', action: 'GitHub' }}
            >
              <I18n path="settings.about.github">
                <Icon type="fontawesome" name="github" />
                {' '}
              </I18n>
            </Button>
          </div>

        </section>
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
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Settings);

import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Header from '../components/Header';
import Icon from '../components/Icon';
import Button from '../components/Button';
import * as uiActions from '../services/ui/actions';
import * as pusherActions from '../services/pusher/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as spotifyActions from '../services/spotify/actions';
import * as snapcastActions from '../services/snapcast/actions';
import { I18n, i18n } from '../locale';

class Debug extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      mopidy_call: 'playlists.asList',
      mopidy_data: '{}',
      pusher_data: '{"method":"get_config"}',
      snapcast_data: '{"method":"Server.GetStatus"}',
      access_token: this.props.access_token,
      toggling_test_mode: false,
    };
  }

  componentDidMount() {
    const { uiActions: { setWindowTitle } } = this.props;
    setWindowTitle(i18n('debug.title'));
  }

  onBack = () => {
    const { history } = this.props;
    history.push('/settings');
  }

  callMopidy = (e) => {
    const { mopidyActions: { debug } } = this.props;
    const { mopidy_call, mopidy_data } = this.state;
    e.preventDefault();
    debug(mopidy_call, JSON.parse(mopidy_data));
  }

  callPusher = (e) => {
    const { pusherActions: { debug } } = this.props;
    const { pusher_data } = this.state;
    e.preventDefault();
    debug(JSON.parse(pusher_data));
  }

  callSnapcast = (e) => {
    const { snapcastActions: { debug } } = this.props;
    const { snapcast_data } = this.state;
    e.preventDefault();
    debug(JSON.parse(snapcast_data));
  }

  toggleTestMode = (e) => {
    const { uiActions: { set }, test_mode } = this.props;
    this.setState({ toggling_test_mode: true });
    set({ test_mode: !test_mode });

    // Wait a second to allow state to update, and then refresh
    setTimeout(location.reload(), 1000);
  }

  render = () => {
    const {
      uiActions,
      spotifyActions,
      history,
      test_mode,
      debug_response,
      debug_info,
      log_actions,
      log_mopidy,
      log_pusher,
      log_snapcast,
      access_token,
    } = this.props;
    const {
      toggling_test_mode,
      mopidy_call,
      mopidy_data,
      pusher_data,
      snapcast_data,
    } = this.state;

    const options = (
      <Button
        onClick={this.onBack}
        noHover
        discrete
      >
        <Icon name="keyboard_backspace" />
        <I18n path="actions.back" />
      </Button>
    );

    return (
      <div className="view debugger-view">
        <Header options={options} uiActions={uiActions}>
          <Icon name="settings" type="material" />
					<I18n path="debug.title" />
        </Header>

        <div className="content-wrapper">

          <h4 className="underline"><I18n path="debug.ui.title" /></h4>
          <form>
            <div className="field checkbox">
              <div className="name">Test mode</div>
              <div className="input">
                <Button
                  type={test_mode ? 'destructive' : null}
                  working={toggling_test_mode}
                  onClick={this.toggleTestMode}
                  tracking={{ category: 'Debug', action: 'TestMode', label: test_mode ? 'Disable' : 'Enable' }}
                >
                  <I18n path={`actions.${test_mode ? 'disable' : 'enable'}`} />
                </Button>
              </div>
            </div>
            <div className="field checkbox">
              <div className="name"><I18n path="debug.debug.title" /></div>
              <div className="input">
                <label>
                  <input
                    type="checkbox"
                    name="debug_info"
                    checked={debug_info}
                    onChange={() => uiActions.set({ debug_info: !debug_info })}
                  />
                  <span className="label"><I18n path="debug.debug.overlay" /></span>
                </label>
              </div>
            </div>
            <div className="field checkbox">
              <div className="name"><I18n path="debug.logging.title" /></div>
              <div className="input">
                <label>
                  <input
                    type="checkbox"
                    name="log_actions"
                    checked={log_actions}
                    onChange={() => uiActions.set({ log_actions: !log_actions })}
                  />
                  <span className="label"><I18n path="debug.logging.actions" /></span>
                </label>
                <label>
                  <input
                    type="checkbox"
                    name="log_mopidy"
                    checked={log_mopidy}
                    onChange={() => uiActions.set({ log_mopidy: !log_mopidy })}
                  />
                  <span className="label"><I18n path="debug.logging.mopidy" /></span>
                </label>
                <label>
                  <input
                    type="checkbox"
                    name="log_pusher"
                    checked={log_pusher}
                    onChange={() => uiActions.set({ log_pusher: !log_pusher })}
                  />
                  <span className="label"><I18n path="debug.logging.pusher" /></span>
                </label>
                <label>
                  <input
                    type="checkbox"
                    name="log_pusher"
                    checked={log_snapcast}
                    onChange={() => uiActions.set({ log_snapcast: !log_snapcast })}
                  />
                  <span className="label"><I18n path="debug.logging.snapcast" /></span>
                </label>
              </div>
            </div>
            <div className="field">
              <div className="name" />
              <div className="input">
                <Button
                  onClick={() => this.props.uiActions.createNotification({ content: 'Test notification' })}
                  tracking={{ category: 'Debug', action: 'Test notification' }}
                >
                  <I18n path="debug.create_notification" />
                </Button>
                <Button
                  onClick={
                    () => this.props.uiActions.startProcess(
                      'TEST_PROCESS',
                      { content: 'Test process', remaining: 68, total: 100 },
                    )
                  }
                  tracking={{ category: 'Debug', action: 'Test process' }}
                >
                  <I18n path="debug.create_process" />
                </Button>
                <Button
                  onClick={() => this.props.pusherActions.request('test')}
                  tracking={{ category: 'Debug', action: 'Run test process' }}
                >
                  <I18n path="debug.run_test" />
                </Button>
              </div>
            </div>
          </form>

          <h4 className="underline">Spotify</h4>
          <label className="field">
            <div className="name"><I18n path="debug.access_token" /></div>
            <div className="input">
              <input
                type="text"
                onChange={(e) => spotifyActions.authorizationGranted({ access_token: e.target.value })}
                value={access_token}
              />
            </div>
          </label>

          <h4 className="underline"><I18n path="services.mopidy.title" /></h4>
          <form onSubmit={this.callMopidy}>
            <label className="field">
              <div className="name"><I18n path="debug.call" /></div>
              <div className="input">
                <input
                  type="text"
                  onChange={(e) => this.setState({ mopidy_call: e.target.value })}
                  value={mopidy_call}
                />
              </div>
            </label>
            <label className="field">
              <div className="name"><I18n path="debug.data" /></div>
              <div className="input">
                <textarea
                  onChange={(e) => this.setState({ mopidy_data: e.target.value })}
                  value={mopidy_data}
                />
              </div>
            </label>
            <div className="field">
              <div className="name" />
              <div className="input">
                <Button
                  submit
                  tracking={{ category: 'Debug', action: 'Mopidy', label: 'Send' }}
                >
                  <I18n path="actions.send" />
                </Button>
              </div>
            </div>
          </form>

          <h4 className="underline"><I18n path="services.pusher.title" /></h4>
          <form onSubmit={this.callPusher}>
            <label className="field">
              <div className="name"><I18n path="debug.examples" /></div>
              <div className="input">
                <select onChange={(e) => this.setState({ pusher_data: e.target.value })}>
                  <option value='{"method":"get_config"}'>Get config</option>
                  <option value='{"method":"get_version"}'>Get version</option>
                  <option value='{"method":"get_connections"}'>Get connections</option>
                  <option value='{"method":"get_radio"}'>Get radio</option>
                  <option value='{"method":"get_queue_metadata"}'>Get queue metadata</option>
                  <option value='{"method":"broadcast","data":{"method":"notification","params":{"notification":{"type":"info","title":"Testing","content":"This is my message"}}}}'>Broadcast to all clients</option>
                  <option value='{"method":"send_message","data":{"method":"notification","params":{"notification":{"type":"info","title":"Testing","content":"This is my message"}}}}'>Broadcast to one client</option>
                  <option value='{"method":"set_username","data":{"connection_id":"CONNECTION_ID_HERE","username":"NewUsername"}}'>Change username</option>
                  <option value='{"method":"refresh_spotify_token"}'>Refresh Spotify token</option>
                  <option value='{"method":"broadcast","data":{"method":"reload","params":{}}}'>Forced reload for all connected clients</option>
                  <option value='{"method":"perform_upgrade"}'>Perform upgrade (beta)</option>
                </select>
              </div>
            </label>
            <label className="field">
              <div className="name"><I18n path="debug.data" /></div>
              <div className="input">
                <textarea
                  onChange={(e) => this.setState({ pusher_data: e.target.value })}
                  value={pusher_data}
                />
              </div>
            </label>
            <div className="field">
              <div className="name" />
              <div className="input">
                <Button
                  submit
                  tracking={{ category: 'Debug', action: 'Pusher', label: 'Send' }}
                >
                  <I18n path="actions.send" />
                </Button>
              </div>
            </div>
          </form>

          <h4 className="underline"><I18n path="services.snapcast.title" /></h4>
          <form onSubmit={(e) => this.callSnapcast(e)}>
            <label className="field">
              <div className="name"><I18n path="debug.data" /></div>
              <div className="input">
                <textarea
                  onChange={(e) => this.setState({ snapcast_data: e.target.value })}
                  value={snapcast_data}
                />
              </div>
            </label>
            <div className="field">
              <div className="name" />
              <div className="input">
                <Button
                  submit
                  tracking={{ category: 'Debug', action: 'Snapcast', label: 'Send' }}
                >
                  <I18n path="actions.send" />
                </Button>
              </div>
            </div>
          </form>

          <h4 className="underline"><I18n path="debug.response" /></h4>
          <pre>
            {debug_response && JSON.stringify(debug_response, null, 2)}
          </pre>

        </div>
      </div>
    );
  }
}

const mapStateToProps = (state) => ({
  connection_id: state.pusher.connection_id,
  access_token: (state.spotify.access_token ? state.spotify.access_token : ''),
  log_actions: (state.ui.log_actions ? state.ui.log_actions : false),
  log_pusher: (state.ui.log_pusher ? state.ui.log_pusher : false),
  log_mopidy: (state.ui.log_mopidy ? state.ui.log_mopidy : false),
  log_snapcast: (state.ui.log_snapcast ? state.ui.log_snapcast : false),
  test_mode: (state.ui.test_mode ? state.ui.test_mode : false),
  debug_info: (state.ui.debug_info ? state.ui.debug_info : false),
  debug_response: state.ui.debug_response,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  pusherActions: bindActionCreators(pusherActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
  snapcastActions: bindActionCreators(snapcastActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Debug);

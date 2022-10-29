import React, { useEffect, useState } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Header from '../components/Header';
import Icon from '../components/Icon';
import Button from '../components/Button';
import LinksSentence from '../components/LinksSentence';
import {
  set,
  setWindowTitle,
  startProcess,
  createNotification,
} from '../services/ui/actions';
import {
  debug as pusherDebug,
  request,
} from '../services/pusher/actions';
import { debug as mopidyDebug } from '../services/mopidy/actions';
import { debug as snapcastDebug } from '../services/snapcast/actions';
import { I18n, i18n } from '../locale';
import { useSelector } from 'react-redux';
import { useDispatch } from 'react-redux';
import { useNavigate } from 'react-router-dom';

const Debug = () => {
  const navigate = useNavigate();
  const dispatch = useDispatch();
  const {
    debug_info,
    log_actions,
    log_mopidy,
    log_pusher,
    log_snapcast,
    debug_response,
  } = useSelector((state) => state.ui);
  const uri_schemes = useSelector((state) => state.mopidy.uri_schemes);
  const [form, setForm] = useState({
    mopidy_call: 'playlists.asList',
    mopidy_data: '{}',
    pusher_data: '{"method":"get_config"}',
    snapcast_data: '{"method":"Server.GetStatus"}',
  });

  useEffect(() => {
    setWindowTitle(i18n('debug.title'));
  }, []);

  const onInputChange = ({ target: { name, value } }) =>
    setForm((prev) => ({ ...prev, [name]: value }));

  const callMopidy = (e) => {
    const { mopidy_call, mopidy_data } = form;
    e.preventDefault();
    dispatch(mopidyDebug(mopidy_call, JSON.parse(mopidy_data)));
  }

  const callPusher = (e) => {
    const { pusher_data } = form;
    e.preventDefault();
    dispatch(pusherDebug(JSON.parse(pusher_data)));
  }

  const callSnapcast = (e) => {
    const { snapcast_data } = form;
    e.preventDefault();
    dispatch(snapcastDebug(JSON.parse(snapcast_data)));
  }

  const options = (
    <Button
      onClick={() => navigate('/settings')}
      noHover
      discrete
    >
      <Icon name="keyboard_backspace" />
      <I18n path="actions.back" />
    </Button>
  );

  return (
    <div className="view debugger-view">
      <Header options={options}>
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
                href={`?test_mode=${test_mode ? '0' : '1'}`}
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
                  onChange={() => dispatch(set({ debug_info: !debug_info }))}
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
                  onChange={() => dispatch(set({ log_actions: !log_actions }))}
                />
                <span className="label"><I18n path="debug.logging.actions" /></span>
              </label>
              <label>
                <input
                  type="checkbox"
                  name="log_mopidy"
                  checked={log_mopidy}
                  onChange={() => dispatch(set({ log_mopidy: !log_mopidy }))}
                />
                <span className="label"><I18n path="debug.logging.mopidy" /></span>
              </label>
              <label>
                <input
                  type="checkbox"
                  name="log_pusher"
                  checked={log_pusher}
                  onChange={() => dispatch(set({ log_pusher: !log_pusher }))}
                />
                <span className="label"><I18n path="debug.logging.pusher" /></span>
              </label>
              <label>
                <input
                  type="checkbox"
                  name="log_pusher"
                  checked={log_snapcast}
                  onChange={() => dispatch(set({ log_snapcast: !log_snapcast }))}
                />
                <span className="label"><I18n path="debug.logging.snapcast" /></span>
              </label>
            </div>
          </div>
          <div className="field">
            <div className="name" />
            <div className="input">
              <Button
                onClick={() => dispatch(createNotification({ content: 'Test notification' }))}
                tracking={{ category: 'Debug', action: 'Test notification' }}
              >
                <I18n path="debug.create_notification" />
              </Button>
              <Button
                onClick={
                  () => dispatch(
                    startProcess(
                      'TEST_PROCESS',
                      { content: 'Test process', remaining: 36, total: 100 },
                    )
                  )
                }
                tracking={{ category: 'Debug', action: 'Test process' }}
              >
                <I18n path="debug.create_process" />
              </Button>
              <Button
                onClick={() => dispatch(request('test'))}
                tracking={{ category: 'Debug', action: 'Run test process' }}
              >
                <I18n path="debug.run_test" />
              </Button>
            </div>
          </div>
        </form>

        <h4 className="underline"><I18n path="services.mopidy.title" /></h4>
        <label className="field">
          <div className="name">Enabled sources</div>
          <div className="input">
            <span className="text">
              <LinksSentence items={uri_schemes.map((name) => ({ name, uri: name }))} nolinks />
            </span>
          </div>
        </label>
        <form onSubmit={callMopidy}>
          <label className="field">
            <div className="name"><I18n path="debug.call" /></div>
            <div className="input">
              <input
                type="text"
                name="mopidy_call"
                onChange={onInputChange}
                value={form.mopidy_call}
              />
            </div>
          </label>
          <label className="field">
            <div className="name"><I18n path="debug.data" /></div>
            <div className="input">
              <textarea
                name="mopidy_data"
                onChange={onInputChange}
                value={form.mopidy_data}
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
        <form onSubmit={callPusher}>
          <label className="field">
            <div className="name"><I18n path="debug.examples" /></div>
            <div className="input">
              <select
                name="pusher_call"
                onChange={onInputChange}
              >
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
                name="pusher_data"
                onChange={onInputChange}
                value={form.pusher_data}
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
        <form onSubmit={callSnapcast}>
          <label className="field">
            <div className="name"><I18n path="debug.data" /></div>
            <div className="input">
              <textarea
                name="snapcast_data"
                onChange={onInputChange}
                value={form.snapcast_data}
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

export default Debug;

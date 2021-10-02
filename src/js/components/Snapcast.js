import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import TextField from './Fields/TextField';
import SnapcastGroups from './SnapcastGroups';
import * as uiActions from '../services/ui/actions';
import * as actions from '../services/snapcast/actions';
import { I18n } from '../locale';
import Button from './Button';

const Snapcast = (props) => {
  const {
    actions,
    show_disconnected_clients,
    uiActions,
    match,
    history,
    config,
    snapcast: {
      host,
      port,
      enabled,
      streaming_enabled,
      connected,
    },
  } = props;

  const restoreDefaults = () => {
    const {
      snapcast_enabled = false,
      snapcast_host = 'localhost',
      snapcast_port = '1780',
    } = config || {};

    actions.setEnabled(snapcast_enabled);
    actions.setConnection({ host: snapcast_host, port: snapcast_port });
  }

  return (
    <div className="snapcast">

      <div className="field checkbox">
        <div className="name">
          <I18n path="snapcast.enabled" />
        </div>
        <div className="input">
          <label>
            <input
              type="checkbox"
              name="enabled"
              checked={enabled}
              onChange={() => actions.setEnabled(!enabled)}
            />
            <span className="label">
              <I18n path="snapcast.enabled" />
            </span>
          </label>
          <label>
            <input
              type="checkbox"
              name="streaming_enabled"
              checked={streaming_enabled}
              disabled={!enabled}
              onChange={() => actions.setStreamingEnabled(!streaming_enabled)}
            />
            <span className="label">
              <I18n path="snapcast.streaming_enabled" />
            </span>
          </label>
          <label>
            <input
              type="checkbox"
              name="show_disconnected_clients"
              checked={show_disconnected_clients}
              onChange={() => uiActions.set({ snapcast_show_disconnected_clients: !show_disconnected_clients })}
            />
            <span className="label">
              <I18n path="snapcast.show_disconnected_clients" />
            </span>
          </label>
          <Button onClick={restoreDefaults} size="small" className="no-margin">
            <I18n path="services.restore_defaults" />
          </Button>
        </div>
      </div>

      <div className="field">
        <div className="name">
          <I18n path="snapcast.host" />
        </div>
        <div className="input">
          <TextField
            value={host}
            onChange={(value) => actions.setConnection({ host: value })}
            autosave
          />
        </div>
      </div>

      <div className="field">
        <div className="name">
          <I18n path="snapcast.port" />
        </div>
        <div className="input">
          <TextField
            value={port}
            name="port"
            onChange={(value) => actions.setConnection({ port: value })}
            autosave
          />
        </div>
      </div>

      {connected && enabled && <SnapcastGroups match={match} history={history} />}
    </div>
  );
};

const mapStateToProps = (state) => ({
  snapcast: state.snapcast,
  config: state.pusher.config,
  show_disconnected_clients: (
    state.ui.snapcast_show_disconnected_clients !== undefined
      ? state.ui.snapcast_show_disconnected_clients
      : false
  ),
});

const mapDispatchToProps = (dispatch) => ({
  actions: bindActionCreators(actions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Snapcast);

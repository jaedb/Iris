
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import TextField from './Fields/TextField';
import SnapcastGroups from './SnapcastGroups';
import * as uiActions from '../services/ui/actions';
import * as actions from '../services/snapcast/actions';

const Snapcast = (props) => {
  const {
    actions,
    show_disconnected_clients,
    uiActions,
    match,
    history,
    snapcast: {
      host,
      port,
      enabled,
      connected,
    },
  } = props;

  return (
    <div className="snapcast">

      <div className="field checkbox">
        <div className="name">Enabled</div>
        <div className="input">
          <label>
            <input
              type="checkbox"
              name="enabled"
              checked={enabled}
              onChange={() => actions.setEnabled(!enabled)}
            />
            <span className="label">
              Enabled
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
              Show disconnected clients
            </span>
          </label>
        </div>
      </div>

      <div className="field">
        <div className="name">Host</div>
        <div className="input">
          <TextField
            value={host}
            onChange={(value) => actions.setConnection({ host: value })}
            autosave
          />
        </div>
      </div>

      <div className="field">
        <div className="name">Port</div>
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
}

const mapStateToProps = (state, ownProps) => ({
  snapcast: state.snapcast,
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

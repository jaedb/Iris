
import React, { memo, useCallback } from 'react';
import { useStore, useDispatch } from 'react-redux'
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Link from './Link';
import Icon from './Icon';
import URILink from './URILink';
import TextField from './Fields/TextField';
import { indexToArray } from '../util/arrays';

import * as uiActions from '../services/ui/actions';
import * as coreActions from '../services/core/actions';
import * as mopidyActions from '../services/mopidy/actions';
import { iconFromKeyword } from '../util/helpers';

const Servers = ({
  match: { params: { id: serverId } },
  history,
}) => {
  const store = useStore();
  const dispatch = useDispatch();
  const {
    mopidy: {
      servers,
      current_server,
      ssl,
      connected: mopidyConnected,
      connecting: mopidyConnecting,
    },
    pusher: {
      connected: pusherConnected,
      connecting: pusherConnecting,
    },
  } = store.getState();

  const addServer = () => {
    const action = mopidyActions.addServer();
    dispatch(action);
    history.push(`/settings/servers/${action.server.id}`);
  };

  const renderMenu = () => (
    <div className="sub-tabs__menu" id="servers-menu">
      <div className="menu__inner">
        {indexToArray(servers).map((server) => {
          let status = <span className="status mid_grey-text">Inactive</span>;
          if (server.id === current_server) {
            if (mopidyConnecting || pusherConnecting) {
              status = <span className="status mid_grey-text">Connecting</span>
            } else if (!mopidyConnected || !pusherConnected) {
              status = <span className="status red-text">Disconnected</span>
            } else if (mopidyConnected && pusherConnected) {
              status = <span className="status green-text">Connected</span>
            }
          }
          return (
            <Link
              history={history}
              className="menu-item"
              activeClassName="menu-item--active"
              to={`/settings/servers/${server.id}`}
              scrollTo="#servers-menu"
              key={server.id}
            >
              <div className="menu-item__inner">
                <Icon className="menu-item__icon" name={iconFromKeyword(server.name) || 'dns'} />
                <div className="menu-item__title">
                  {server.name}
                </div>
                {status}
              </div>
            </Link>
          );
        })}
        <span
          className="menu-item menu-item--add"
          onClick={addServer}
        >
          <div className="menu-item__inner">
            <Icon className="menu-item__icon" name="add" />
            <div className="menu-item__title">
              Add
            </div>
            <span className="status mid_grey-text">New server</span>
          </div>
        </span>
      </div>
    </div>
  );

  const renderServer = () => {
    if (!serverId) return null;
    const server = servers[serverId];
    if (!server) return null;

    const remove = () => {
      dispatch(mopidyActions.removeServer(server.id));
    };

    const setAsCurrent = () => {
      dispatch(mopidyActions.setCurrentServer(server));
    };

    return (
      <div className="sub-tabs__content">
        <label className="field">
          <div className="name">Name</div>
          <div className="input">
            <TextField
              type="text"
              value={server.name}
              onChange={value => dispatch(mopidyActions.updateServer({ id: server.id, name: value }))}
              autosave
            />
          </div>
        </label>
        <label className="field">
          <div className="name">Host</div>
          <div className="input">
            <TextField
              value={server.host}
              onChange={value => dispatch(mopidyActions.updateServer({ id: server.id, host: value }))}
              autosave
            />
          </div>
        </label>
        <label className="field">
          <div className="name">Port</div>
          <div className="input">
            <TextField
              type="text"
              value={server.port}
              onChange={value => dispatch(mopidyActions.updateServer({ id: server.id, port: value }))}
              autosave
            />
          </div>
        </label>

        <div className="field checkbox">
          <div className="name">Encryption</div>
          <div className="input">
            <label>
              <input
                type="checkbox"
                name="ssl"
                value={server.ssl}
                checked={server.ssl}
                onChange={() => dispatch(mopidyActions.updateServer({ id: server.id, ssl: !server.ssl }))}
              />
              <span className="label tooltip">
                Use SSL
                <span className="tooltip__content">Requires a SSL proxy</span>
              </span>
            </label>
          </div>
        </div>

        <button
          type="button"
          className="button button--primary"
          onClick={setAsCurrent}
        >
          Switch to this server
        </button>
        <button
          type="button"
          className="button button--destructive"
          disabled={server.id === current_server}
          onClick={remove}
        >
          Remove
        </button>
      </div>
    );
  };

  return (
    <div className="sub-tabs sub-tabs--servers">
      {renderMenu()}
      {renderServer()}
    </div>
  );
};

export default Servers;

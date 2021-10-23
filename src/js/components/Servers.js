import React from 'react';
import { Route, Switch, useParams, useHistory } from 'react-router-dom';
import { useSelector, useDispatch } from 'react-redux';
import Link from './Link';
import Icon from './Icon';
import TextField from './Fields/TextField';
import { indexToArray } from '../util/arrays';
import { Button } from './Button';
import * as mopidyActions from '../services/mopidy/actions';
import { iconFromKeyword } from '../util/helpers';
import { I18n } from '../locale';

const Server = () => {
  const { id } = useParams();
  if (!id) return null;

  const dispatch = useDispatch();
  const current_server = useSelector((state) => state.mopidy.current_server);
  const server = useSelector((state) => state.mopidy.servers[id]);
  if (!server) return null;

  const remove = () => dispatch(mopidyActions.removeServer(id));
  const setAsCurrent = () => dispatch(mopidyActions.setCurrentServer(server));
  const isCurrent = id === current_server;

  return (
    <div className="sub-tabs__content">
      <label className="field">
        <div className="name">
          <I18n path="settings.servers.name" />
        </div>
        <div className="input">
          <TextField
            type="text"
            value={server.name}
            onChange={(value) => dispatch(mopidyActions.updateServer({ id, name: value }))}
            autosave
          />
        </div>
      </label>
      <label className="field">
        <div className="name">
          <I18n path="settings.servers.host" />
        </div>
        <div className="input">
          <TextField
            value={server.host}
            onChange={(value) => dispatch(mopidyActions.updateServer({ id, host: value }))}
            autosave
          />
        </div>
      </label>
      <label className="field">
        <div className="name">
          <I18n path="settings.servers.port" />
        </div>
        <div className="input">
          <TextField
            type="text"
            value={server.port}
            onChange={(value) => dispatch(mopidyActions.updateServer({ id, port: value }))}
            autosave
          />
        </div>
      </label>

      <div className="field checkbox">
        <div className="name">
          <I18n path="settings.servers.encryption.label" />
        </div>
        <div className="input">
          <label>
            <input
              type="checkbox"
              name="ssl"
              value={server.ssl}
              checked={server.ssl}
              onChange={() => dispatch(mopidyActions.updateServer({ id, ssl: !server.ssl }))}
            />
            <span className="label tooltip">
              <I18n path="settings.servers.encryption.sublabel" />
              <span className="tooltip__content">
                <I18n path="settings.servers.encryption.description" />
              </span>
            </span>
            {!server.ssl && window.location.protocol === 'https:' && (
              <span className="red-text">
                <I18n path="settings.servers.encryption.incompatible" />
              </span>
            )}
          </label>
        </div>
      </div>

      <Button
        type={isCurrent ? 'default' : 'primary'}
        onClick={setAsCurrent}
        tracking={{
          category: 'Servers',
          action: 'SetAsCurrent',
          label: (isCurrent ? 'Reconnect' : 'Switch'),
        }}
      >
        <I18n path={`settings.servers.${isCurrent ? 'reconnect' : 'switch'}`} />
      </Button>
      <Button
        type="destructive"
        disabled={isCurrent}
        onClick={remove}
        tracking={{ category: 'Servers', action: 'Delete' }}
      >
        <I18n path="actions.remove" />
      </Button>
    </div>
  );
};

const Menu = () => {
  const history = useHistory();
  const dispatch = useDispatch();
  const servers = indexToArray(useSelector((state) => state.mopidy.servers));

  const {
    current_server,
    connected: mopidyConnected,
    connecting: mopidyConnecting,
  } = useSelector((state) => state.mopidy);
  const {
    connected: pusherConnected,
    connecting: pusherConnecting,
  } = useSelector((state) => state.pusher);

  const addServer = () => {
    const action = mopidyActions.addServer();
    dispatch(action);
    history.push(`/settings/servers/${action.server.id}`);
  };

  return (
    <div className="sub-tabs__menu" id="servers-menu">
      <div className="menu__inner">
        {servers.map((server) => {
          let status = (
            <span className="status mid_grey-text">
              <I18n path="settings.servers.inactive" />
            </span>
          );
          if (server.id === current_server) {
            if (mopidyConnecting || pusherConnecting) {
              status = (
                <span className="status mid_grey-text">
                  <I18n path="settings.servers.connecting" />
                </span>
              );
            } else if (!mopidyConnected || !pusherConnected) {
              status = (
                <span className="status red-text">
                  <I18n path="settings.servers.disconnected" />
                </span>
              );
            } else if (mopidyConnected && pusherConnected) {
              status = (
                <span className="status green-text">
                  <I18n path="settings.servers.connected" />
                </span>
              );
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
              <I18n path="actions.add" />
            </div>
            <span className="status mid_grey-text">
              <I18n path="settings.servers.new_server" />
            </span>
          </div>
        </span>
      </div>
    </div>
  );
};

const Servers = () => (
  <div className="sub-tabs sub-tabs--servers">
    <Menu />
    <Switch>
      <Route
        exact
        path="/settings/servers/:id"
        component={Server}
      />
    </Switch>
  </div>
);

export default Servers;

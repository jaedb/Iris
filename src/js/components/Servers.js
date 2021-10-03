import React, { useState, useEffect } from 'react';
import { useStore, useDispatch } from 'react-redux';
import Link from './Link';
import Icon from './Icon';
import TextField from './Fields/TextField';
import { indexToArray } from '../util/arrays';
import { Button } from './Button';
import * as mopidyActions from '../services/mopidy/actions';
import { iconFromKeyword } from '../util/helpers';
import { I18n } from '../locale';
import LinksSentence from './LinksSentence';
import {
  digestMopidyImages,
  formatImages,
} from '../util/format';
import Thumbnail from './Thumbnail';

const callServer = ({ endpoint, method, params }) => new Promise((resolve, reject) => {
  const body = {
    jsonrpc: '2.0',
    id: 1,
    method,
    params,
  };
  fetch(endpoint, {
    method: 'POST',
    mode: 'no-cors',
    credentials: 'same-origin',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  })
    .then((response) => response.json())
    .then(({ result }) => resolve(result))
    .catch((error) => { console.debug({ error }); reject() });
});

const Server = ({
  server,
  current_server,
}) => {
  if (!server) return null;
  const { id } = server;
  const dispatch = useDispatch();
  const [currentTrack, setCurrentTrack] = useState();
  const [playState, setPlayState] = useState();

  useEffect(() => {
    const endpoint = `http://${server.host}:${server.port}/mopidy/rpc`;
    callServer({
      endpoint,
      method: 'core.playback.get_current_track',
    }).then(
      (track) => {
        callServer({
          endpoint,
          method: 'core.library.get_images',
          params: {
            uris: [track.uri],
          },
        }).then(({ [track.uri]: images }) => {
          console.debug({ track })
          setCurrentTrack({
            ...track,
            images: images ? formatImages(digestMopidyImages(server, images)) : null,
          });
        });
      },
      () => setCurrentTrack(null),
    );

    callServer({
      endpoint,
      method: 'core.playback.get_state',
    }).then(setPlayState, setPlayState);
  }, [id]);

  const remove = () => {
    dispatch(mopidyActions.removeServer(server.id));
  };

  const setAsCurrent = () => {
    dispatch(mopidyActions.setCurrentServer(server));
  };

  return (
    <div className="sub-tabs__content">
      <label className="field">
        <div className="name">
          <I18n path="settings.servers.current_track" />
          {playState ? ` (${playState})` : ''}
        </div>
        <div className="input">
          <div style={{ display: 'flex', alignItems: 'center' }}>
            <Thumbnail images={currentTrack?.images} size="small" />
            <div style={{ paddingLeft: '1rem' }}>
              <div>{currentTrack?.name}</div>
              <em>
                <LinksSentence items={currentTrack?.artists} type="artist" nolinks />
              </em>
            </div>
          </div>
        </div>
      </label>
      <label className="field">
        <div className="name">
          <I18n path="settings.servers.name" />
        </div>
        <div className="input">
          <TextField
            type="text"
            value={server.name}
            onChange={(value) => dispatch(mopidyActions.updateServer({ id: server.id, name: value }))}
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
            onChange={(value) => dispatch(mopidyActions.updateServer({ id: server.id, host: value }))}
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
            onChange={(value) => dispatch(mopidyActions.updateServer({ id: server.id, port: value }))}
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
              onChange={() => dispatch(mopidyActions.updateServer({ id: server.id, ssl: !server.ssl }))}
            />
            <span className="label tooltip">
              <I18n path="settings.servers.encryption.sublabel" />
              <span className="tooltip__content">
                <I18n path="settings.servers.encryption.description" />
              </span>
            </span>
          </label>
        </div>
      </div>

      <Button
        type="primary"
        onClick={setAsCurrent}
        tracking={{
          category: 'Servers',
          action: 'SetAsCurrent',
          label: (server.id === current_server ? 'Reconnect' : 'Switch'),
        }}
      >
        <I18n path={`settings.servers.${server.id === current_server ? 'reconnect' : 'switch'}`} />
      </Button>
      <Button
        type="destructive"
        disabled={server.id === current_server}
        onClick={remove}
        tracking={{ category: 'Servers', action: 'Delete' }}
      >
        <I18n path="actions.remove" />
      </Button>
    </div>
  );
};

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

  const Menu = () => (
    <div className="sub-tabs__menu" id="servers-menu">
      <div className="menu__inner">
        {indexToArray(servers).map((server) => {
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

  return (
    <div className="sub-tabs sub-tabs--servers">
      <Menu />
      <Server
        server={servers[serverId]}
        current_server={current_server}
      />
    </div>
  );
};

export default Servers;

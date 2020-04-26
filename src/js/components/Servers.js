
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

const Servers = ({
  match: { params: { id: serverId } },
  history,
}) => {
  const store = useStore();
  const dispatch = useDispatch();
  const { mopidy: { servers } } = store.getState();

  const renderMenu = () => (
    <div className="sub-tabs__menu" id="servers-menu">
      <div className="menu__inner">
        {indexToArray(servers).map((server) => (
          <Link
            history={history}
            className="menu-item"
            activeClassName="menu-item--active"
            to={`/settings/servers/${server.id}`}
            scrollTo="#servers-menu"
            key={server.id}
          >
            <div className="menu-item__inner">
              <div className="menu-item__title">
                {server.name}
              </div>
            </div>
          </Link>
        ))}
      </div>
    </div>
  );

  const renderServer = () => {
    if (!serverId) return null;
    const server = servers[serverId];
    if (!server) return null;

    return (
      <div className="sub-tabs__content">
        <label className="field">
          <div className="name">Name</div>
          <div className="input">
            <TextField
              type="text"
              value={server.name}
              onChange={value => dispatch(mopidyActions.updateServer({ id: server.id, name: value }))}
            />
          </div>
        </label>
        <label className="field">
          <div className="name">Host</div>
          <div className="input">
            <TextField
              value={server.host}
              onChange={value => updateServer({ id: server.id, host: value })}
            />
          </div>
        </label>
        <label className="field">
          <div className="name">Port</div>
          <div className="input">
            <TextField
              type="text"
              value={server.port}
              onChange={value => updateServer({ id: server.id, port: value })}
            />
          </div>
        </label>
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


import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import VolumeControl from './Fields/VolumeControl';
import MuteControl from './Fields/MuteControl';
import TextField from './Fields/TextField';
import SnapcastClients from './SnapcastClients';
import Icon from './Icon';
import Link from './Link';

import * as helpers from '../helpers';
import * as actions from '../services/snapcast/actions';

const SnapcastGroups = (props) => {
  const {
    actions,
    show_disconnected_clients,
    streams,
    groups,
    clients,
    history,
    match: { params: { id: groupId } },
  } = props;

  const streamsArray = Object.keys(streams).map((id) => streams[id]);
  const groupsArray = Object.keys(groups).map((id) => groups[id]);

  if (!groups || groupsArray.length <= 0) {
    return null;
  }

  const renderGroup = () => {
    if (!groupId || !groups[groupId]) {
      return null;
    }

    const group = helpers.collate(groups[groupId], { clients });

    // Average our clients' volume for an overall group volume
    let group_volume = 0;
    for (let i = 0; i < group.clients.length; i++) {
      const client = group.clients[i];
      group_volume += client.volume;
    }
    group_volume /= group.clients.length;

    return (
      <div className="snapcast__group" key={group.id}>
        <div className="field text">
          <div className="name">
            Name
          </div>
          <div className="input">
            <TextField
              value={group.name}
              onChange={value => actions.setGroupName(group.id, value)}
            />
          </div>
        </div>
        <div className="field dropdown">
          <div className="name">
            Stream
          </div>
          <div className="input">
            <select onChange={(e) => actions.setGroupStream(group.id, e.target.value)} value={group.stream_id}>
              {
                streamsArray.map((stream) => (
                  <option value={stream.id} key={stream.id}>
                    {stream.id}
                    {' '}
                    (
                      {stream.status}
                    )
                  </option>
                ))
              }
            </select>
          </div>
        </div>
        <div className="field">
          <div className="name">
            Volume
          </div>
          <div className="input">
            <MuteControl
              className="snapcast__group__mute-control"
              mute={group.muted}
              onMuteChange={(mute) => actions.setGroupMute(group.id, mute)}
            />
            <VolumeControl
              className="snapcast__group__volume-control"
              volume={group_volume}
              onVolumeChange={(percent, previousPercent) => actions.setGroupVolume(group.id, percent, previousPercent)}
            />
          </div>
        </div>
        <div className="field">
          <div className="name">
            Clients
          </div>
          <div className="input">
            <SnapcastClients
              group={group}
              groups={groupsArray}
              actions={actions}
              show_disconnected_clients={show_disconnected_clients}
            />
          </div>
        </div>
      </div>
    );
  }

  const renderMenuItem = (group) => {
    return (
      <Link
        className="snapcast__groups__menu-item menu-item"
        activeClassName="menu-item--active"
        key={group.id}
        history={history}
        to={`/settings/snapcast/${group.id}`}
        scrollTo="#services-snapcast-groups"
      >
        <div className="snapcast__groups__menu-item__inner menu-item__inner">
          <Icon className="menu-item__icon" name="wifi_tethering" />
          <div className="menu-item__title">
            {group.name}
          </div>
          <MuteControl
            className="snapcast__mute-control snapcast__group__menu-item__mute-control"
            mute={group.mute}
            onMuteChange={(mute) => actions.setGroupMute(group.id, mute)}
          />
        </div>
      </Link>
    );
  }

  return (
    <div className="snapcast__groups">
      <a name="services-snapcast-groups" />
      <div className="snapcast__groups__menu menu">
        <div className="menu__inner">
          {groupsArray.map((group) => renderMenuItem(group))}
        </div>
      </div>
      {renderGroup()}
    </div>
  );
}

const mapStateToProps = (state, ownProps) => ({
  show_disconnected_clients: (
    state.ui.snapcast_show_disconnected_clients !== undefined
      ? state.ui.snapcast_show_disconnected_clients
      : false
  ),
  streams: (state.snapcast.streams ? state.snapcast.streams : null),
  groups: (state.snapcast.groups ? state.snapcast.groups : null),
  clients: (state.snapcast.clients ? state.snapcast.clients : null),
});

const mapDispatchToProps = (dispatch) => ({
  actions: bindActionCreators(actions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(SnapcastGroups);


import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { collate } from '../util/format';
import { sortItems } from '../util/arrays';
import { iconFromKeyword } from '../util/helpers';
import VolumeControl from './Fields/VolumeControl';
import MuteControl from './Fields/MuteControl';
import TextField from './Fields/TextField';
import SnapcastClients from './SnapcastClients';
import Icon from './Icon';
import Link from './Link';
import * as actions from '../services/snapcast/actions';
import SelectField from './Fields/SelectField';

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

  const group = groupId && groups[groupId]
    ? collate(groups[groupId], { clients })
    : null;

  const renderGroup = () => {
    if (!group) return null;

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
              autosave
            />
          </div>
        </div>
        <div className="field dropdown">
          <div className="name">
            Stream
          </div>
          <div className="input">
            <SelectField
              onChange={(value) => actions.setGroupStream(group.id, value)}
              value={group.stream_id}
              options={streamsArray.map((stream) => (
                {
                  key: `group_${group.id}_stream_${stream.id}`,
                  value: stream.id,
                  label: `${stream.id} (${stream.status})`,
                }
              ))}
              autosave
            />
          </div>
        </div>
        <div className="field">
          <div className="name">
            Volume
          </div>
          <div className="input">
            <MuteControl
              className="snapcast__group__mute-control snapcast__mute-control"
              mute={group.mute}
              onMuteChange={(mute) => actions.setGroupMute(group.id, mute)}
            />
            <VolumeControl
              className="snapcast__group__volume-control snapcast__volume-control"
              volume={group.volume}
              mute={group.mute}
              onVolumeChange={(percent, previousPercent) => actions.setGroupVolume(group.id, percent, previousPercent)}
            />
          </div>
        </div>
        <SnapcastClients
          group={group}
          groups={groupsArray}
          actions={actions}
          show_disconnected_clients={show_disconnected_clients}
        />
      </div>
    );
  }

  const renderMenuItem = (simpleGroup) => {
    const group = collate(simpleGroup, { clients });
    const anyClients = (
      !show_disconnected_clients && (
        !group.clients ||
        !group.clients.length ||
        !group.clients.filter((client) => client.connected).length
      )
    );
    return (
      <Link
        className={`snapcast__groups__menu-item menu-item${anyClients ? ' menu-item--no-clients' : ''}`}
        activeClassName="menu-item--active"
        key={group.id}
        history={history}
        to={`/settings/services/snapcast/${group.id}`}
        scrollTo="#services-snapcast-groups"
      >
        <div className="snapcast__groups__menu-item__inner menu-item__inner">
          <Icon className="menu-item__icon" name={iconFromKeyword(group.name.toLowerCase()) || 'speaker_group'} />
          <div className="menu-item__title">
            {group.name}
            {group.mute && (
              <Icon name="volume_off" />
            )}
          </div>
        </div>
      </Link>
    );
  }

  return (
    <div className="sub-tabs snapcast__groups" id="services-snapcast-groups">
      <div className="sub-tabs__menu snapcast__groups__menu menu">
        <div className="menu__inner">
          {sortItems(groupsArray, 'name').map((group) => renderMenuItem(group))}
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

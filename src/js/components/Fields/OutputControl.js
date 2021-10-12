import React, { useState, useEffect } from 'react';
import { connect, useSelector, useDispatch } from 'react-redux';
import { bindActionCreators } from 'redux';
import VolumeControl from './VolumeControl';
import MuteControl from './MuteControl';
import Icon from '../Icon';
import DropdownField from './DropdownField';
import * as coreActions from '../../services/core/actions';
import * as pusherActions from '../../services/pusher/actions';
import * as snapcastActions from '../../services/snapcast/actions';
import { sortItems, indexToArray, applyFilter } from '../../util/arrays';
import { collate } from '../../util/format';
import { I18n } from '../../locale';

const SnapcastGroups = () => {
  const dispatch = useDispatch();
  const showAllClients = useSelector((state) => state.ui.snapcast_show_disconnected_clients);
  const clients = useSelector((state) => state.snapcast.clients);
  const groupsObj = useSelector((state) => state.snapcast.groups);
  const streamsObj = useSelector((state) => state.snapcast.streams);

  const groups = indexToArray(groupsObj);
  if (groups.length <= 0) return null;

  const streams = Object.keys(streamsObj).map((id) => ({ value: id, label: id }));

  return (
    <div>
      {
        sortItems(groups, 'name').map((simpleGroup) => {
          const group = collate(simpleGroup, { clients });
          let { clients: groupClients = [] } = group;
          if (!showAllClients) {
            groupClients = applyFilter('connected', true, groupClients);
          }

          if (!groupClients.length) return null;

          const volume = groupClients.reduce(
            (acc, client) => acc + (client.volume || 0),
            0,
          ) / groupClients.length;

          return (
            <div className="output-control__item outputs__item--snapcast" key={group.id}>
              <div className="output-control__item__name">
                {group.name}
              </div>
              <div className="output-control__item__controls">
                <DropdownField
                  name="Source"
                  value={group.stream_id}
                  icon="settings_input_component"
                  options={streams}
                  noLabel
                  handleChange={(value) => dispatch(snapcastActions.setGroupStream(group.id, value))}
                />
                <MuteControl
                  className="output-control__item__mute"
                  noTooltip
                  mute={group.mute}
                  onMuteChange={(mute) => dispatch(snapcastActions.setGroupMute(group.id, mute))}
                />
                <VolumeControl
                  className="output-control__item__volume"
                  volume={volume}
                  mute={group.mute}
                  onVolumeChange={
                    (percent, previousPercent) => dispatch(
                      snapcastActions.setGroupVolume(group.id, percent, previousPercent)
                    )
                  }
                />
              </div>
            </div>
          );
        })
      }
    </div>
  );
}

const Commands = () => {
  const dispatch = useDispatch();
  const commandsObj = useSelector((state) => state.pusher.commands || {});
  if (!commandsObj) return null;

  let items = indexToArray(commandsObj);
  if (items.length <= 0) return null;

  items = sortItems(items, 'sort_order');

  return (
    <div className="output-control__item output-control__item--commands commands">
      {
        items.map((command) => (
          <div
            key={command.id}
            className="commands__item commands__item--interactive"
            onClick={() => dispatch(pusherActions.runCommand(command.id))}
          >
            <Icon className="commands__item__icon" name={command.icon} />
            <span className={`${command.colour}-background commands__item__background`} />
          </div>
        ))
      }
    </div>
  );
}

const Outputs = () => {
  const snapcastGroups = <SnapcastGroups />;
  const commands = <Commands />;

  if (!snapcastGroups && !commands) {
    return (
      <div className="output-control__items output-control__items--no-results">
        <p className="no-results">
          <I18n path="playback_controls.no_outputs" />
        </p>
      </div>
    );
  }
  return (
    <div className="output-control__items">
      {commands}
      {snapcastGroups}
    </div>
  );
}

const OutputControl = ({
  force_expanded,
  pusher_commands,
  snapcast_enabled,
}) => {
  const [expanded, setExpanded] = useState(false);
  const handleClick = (e) => {
    if (!force_expanded && $(e.target).closest('.output-control').length <= 0) {
      setExpanded(false);
    }
  };

  useEffect(() => {
    if (force_expanded && !expanded) {
      setExpanded(true);
    }
  }, [force_expanded]);

  useEffect(() => {
    if (expanded) {
      window.addEventListener('click', handleClick, false);
    } else {
      window.removeEventListener('click', handleClick, false);
    }
  }, [expanded]);

  if (expanded) {
    return (
      <span className="output-control">
        <button
          className="control speakers active"
          onClick={() => setExpanded(false)}
        >
          <Icon name="speaker" />
        </button>
        <Outputs />
      </span>
    );
  }

  // No customisable outputs
  if (!snapcast_enabled && !pusher_commands) {
    return (
      <span className="output-control disabled">
        <button className="control speakers">
          <Icon name="speaker" />
        </button>
      </span>
    );
  }
  return (
    <span className="output-control">
      <button
        className="control speakers"
        onClick={() => setExpanded(true)}
      >
        <Icon name="speaker" />
      </button>
    </span>
  );
}

const mapStateToProps = (state) => ({
  pusher_connected: state.pusher.connected,
  snapcast_enabled: (state.pusher.config ? state.pusher.config.snapcast_enabled : null),
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  snapcastActions: bindActionCreators(snapcastActions, dispatch),
  pusherActions: bindActionCreators(pusherActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(OutputControl);

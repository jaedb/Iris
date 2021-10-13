import React, { useState, useEffect } from 'react';
import { connect, useSelector, useDispatch } from 'react-redux';
import { bindActionCreators } from 'redux';
import { find, groupBy, map } from 'lodash';
import VolumeControl from './VolumeControl';
import MuteControl from './MuteControl';
import Icon from '../Icon';
import DropdownField from './DropdownField';
import Thumbnail from '../Thumbnail';
import LinksSentence from '../LinksSentence';
import * as coreActions from '../../services/core/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as pusherActions from '../../services/pusher/actions';
import * as snapcastActions from '../../services/snapcast/actions';
import { sortItems, indexToArray, applyFilter } from '../../util/arrays';
import { collate } from '../../util/format';
import { titleCase } from '../../util/helpers';
import { I18n } from '../../locale';

const Header = ({ stream, server }) => {
  const dispatch = useDispatch();
  if (server) {
    const {
      id,
      name,
      current_track,
      playback_state,
    } = server;
    const current_server = useSelector((state) => state.mopidy.current_server);
    let stateIcon = '';
    switch (playback_state) {
      case 'playing':
        stateIcon = 'play_arrow';
        break;
      case 'paused':
        stateIcon = 'pause';
        break;
      default:
        stateIcon = 'stop';
        break;
    }
    return (
      <div className="output-control__output__header">
        <Thumbnail
          images={current_track?.images}
          size="small"
          className="output-control__output__header__thumbnail"
        />
        <div className="output-control__output__header__content">
          <h5 className="output-control__output__header__title">
            {name}
            <Icon name={stateIcon} />
            {id !== current_server && (
              <span
                className="flag flag--default"
                onClick={() => dispatch(mopidyActions.setCurrentServer(server))}
              >
                SWITCH TO THIS SERVER
              </span>
            )}
          </h5>
          {current_track ? (
            <ul className="details">
              <li>{current_track?.name}</li>
              <li><LinksSentence items={current_track?.artists} type="artist" nolinks /></li>
            </ul>
          ) : (
            <div className="details">
              <I18n path="modal.servers.nothing_playing" />
            </div>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="output-control__output__header">
      No server: {stream?.id}
    </div>
  );
}

const Clients = ({ clients }) => {
  const dispatch = useDispatch();

  return (
    <div className="output-control__output__clients">
      {clients.map((client) => {
        const {
          id,
          name,
          mute,
          volume,
        } = client;
        return (
          <div className="output-control__output__clients__item" key={client.id}>
            <h5 className="output-control__output__clients__item__title">
              {titleCase(name)}
            </h5>
            <div className="output-control__output__clients__item__volume">
              <MuteControl
                noTooltip
                mute={mute}
                onMuteChange={(value) => dispatch(snapcastActions.setClientMute(id, value))}
              />
              <VolumeControl
                volume={volume}
                mute={mute}
                onVolumeChange={
                  (value) => dispatch(snapcastActions.setClientVolume(id, value))
                }
              />
            </div>
          </div>
        );
      })}
    </div>
  );
}

const Group = ({
  item: {
    stream,
    server,
    clients,
  },
}) => {
  return (
    <div className="output-control__output">
      <Header stream={stream} server={server} />
      <Clients clients={clients} />
    </div>
  );
}

const Outputs = () => {
  const dispatch = useDispatch();
  const groups = indexToArray(useSelector((state) => state.snapcast.groups || {}));
  const clients = useSelector((state) => state.snapcast.clients || {});
  const streams = useSelector((state) => state.snapcast.streams || {});
  const servers = indexToArray(useSelector((state) => state.mopidy.servers || {}));
  const groupsByStream = groupBy(groups, 'stream_id');

  useEffect(() => {
    servers.forEach(({ id }) => {
      dispatch(mopidyActions.getServerState(id));
    });
  }, []);

  return (
    <>
      {map(groupsByStream, (groups, stream_id) => {
        const clients_ids = groups.reduce((acc, curr) => [...acc, ...curr.clients_ids], []);
        const group = {
          stream: find(streams, (s) => s.id === stream_id),
          server: find(servers, (s) => s.snapcast_stream === stream_id),
          clients: clients_ids.map((id) => clients[id]).filter((c) => c.connected),
        };
        return <Group item={group} key={stream_id} />;
      })}
    </>
  );
}

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
    <div className="output-control__commands commands">
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
};

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

  if (expanded) {
    const outputs = <Outputs />;
    const commands = <Commands />;
    return (
      <span className="output-control">
        <button
          className="control speakers active"
          onClick={() => setExpanded(false)}
        >
          <Icon name="speaker" />
        </button>
        {!outputs && !commands ? (
          <div className="output-control__inner output-control__inner--no-results">
            <p className="no-results">
              <I18n path="playback_controls.no_outputs" />
            </p>
          </div>
        ) : (
          <div className="output-control__inner">
            {commands}
            {outputs}
          </div>
        )}
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

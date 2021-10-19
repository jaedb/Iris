import React, { useState, useEffect } from 'react';
import { connect, useSelector, useDispatch } from 'react-redux';
import { bindActionCreators } from 'redux';
import { find, groupBy, map } from 'lodash';
import VolumeControl from './VolumeControl';
import MuteControl from './MuteControl';
import Icon from '../Icon';
import Thumbnail from '../Thumbnail';
import LinksSentence from '../LinksSentence';
import DropdownField from './DropdownField';
import * as coreActions from '../../services/core/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as pusherActions from '../../services/pusher/actions';
import * as snapcastActions from '../../services/snapcast/actions';
import { sortItems, indexToArray } from '../../util/arrays';
import { formatImages, digestMopidyImages } from '../../util/format';
import { titleCase } from '../../util/helpers';
import { I18n } from '../../locale';

const Header = ({ stream, server }) => {
  const dispatch = useDispatch();
  const {
    id,
    meta,
    status,
    uri: {
      path,
    },
  } = stream || {};
  let {
    control_url,
    images,
    name,
    artists,
  } = meta || {};
  const controlURL = meta?.control_url ? new URL(meta.control_url) : null;
  const controlServer = controlURL ? {
    url: meta.control_url,
    ssl: controlURL.protocol === 'https:',
    host: controlURL.hostname,
    port: controlURL.port || (controlURL.protocol === 'https:' ? '443' : '80'),
  } : null;
  const current_server_id = useSelector((state) => state.mopidy.current_server);
  const current_server = useSelector((state) => state.mopidy.servers[current_server_id]);
  const isCurrentServer = control_url === current_server.url;
  const isControlSwitchable = controlServer && !isCurrentServer;
  if (images) images = formatImages(digestMopidyImages(controlServer, images));

  const onClick = () => {
    if (isControlSwitchable) {
      dispatch(mopidyActions.setCurrentServer(controlServer));
    }
  };

  return (
    <div className="output-control__stream__header">
      <Thumbnail
        images={images}
        size="small"
        className="output-control__stream__header__thumbnail"
      />
      <div className="output-control__stream__header__content">
        <h5
          className="output-control__stream__header__title tooltip"
          onClick={onClick}
          style={{ cursor: isControlSwitchable ? 'pointer' : 'default' }}
        >
          {server?.name || id}
          {isCurrentServer && <Icon name="check" />}
          {status === 'playing' && <Icon name="play_arrow" />}
          {control_url && <div className="tooltip__content">{control_url}</div>}
        </h5>
        <ul className="details">
          <li>{name || path}</li>
          {artists && <li><LinksSentence items={artists} type="artist" nolinks /></li>}
        </ul>
      </div>
    </div>
  );
}

const Group = ({
  group: {
    id: groupId,
    name: groupName,
    stream_id,
    clients_ids = [],
  } = {},
}) => {
  const allClients = useSelector((state) => state.snapcast.clients || {});
  const allStreams = indexToArray(useSelector((state) => state.snapcast.streams || {}));
  const clients = clients_ids.length > 0 ? clients_ids.map((c) => allClients[c]) : [];
  const dispatch = useDispatch();

  return (
    <div className="output-control__group">
      <h5 className="output-control__group__title">
        <div className="text">{titleCase(groupName)}</div>
        <DropdownField
          name="Stream"
          value={stream_id}
          icon="settings_input_component"
          options={allStreams.map((s) => ({ value: s.id, label: titleCase(s.id) }))}
          noLabel
          handleChange={
            (value) => dispatch(snapcastActions.setGroupStream(groupId, value))
          }
        />
      </h5>
      <div className="output-control__clients">
        {clients.map((client) => {
          const {
            id: clientId,
            name: clientName,
            mute,
            volume,
          } = client;
          return (
            <div className="output-control__clients__item" key={clientId}>
              <h5 className="output-control__clients__item__title">
                {titleCase(clientName)}
              </h5>
              <div className="output-control__clients__item__volume">
                <MuteControl
                  noTooltip
                  mute={mute}
                  onMuteChange={
                    (value) => dispatch(snapcastActions.setClientMute(clientId, value))
                  }
                />
                <VolumeControl
                  volume={volume}
                  mute={mute}
                  onVolumeChange={
                    (value) => dispatch(snapcastActions.setClientVolume(clientId, value))
                  }
                />
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};

const Outputs = () => {
  const allGroups = indexToArray(useSelector((state) => state.snapcast.groups || {}));
  const allStreams = useSelector((state) => state.snapcast.streams || {});
  const allServers = indexToArray(useSelector((state) => state.mopidy.servers || {}));
  const groupsByStream = groupBy(allGroups, 'stream_id');

  return (
    <>
      {map(groupsByStream, (groups, id) => {
        const stream = {
          id,
          ...find(allStreams, (s) => s.id === id) || {},
          server: find(allServers, (s) => s.snapcast_stream === id),
          groups,
        };
        return (
          <div className="output-control__stream">
            <Header stream={stream} />
            {groups.map((group) => <Group group={group} /> )}
          </div>
        );
      })}
    </>
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

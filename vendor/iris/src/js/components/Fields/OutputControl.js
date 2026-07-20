import React, { useState, useEffect } from 'react';
import { useSelector, useDispatch } from 'react-redux';
import { find, groupBy, map, isEmpty } from 'lodash';
import VolumeControl from './VolumeControl';
import MuteControl from './MuteControl';
import Icon from '../Icon';
import Thumbnail from '../Thumbnail';
import LinksSentence from '../LinksSentence';
import DropdownField from './DropdownField';
import * as pusherActions from '../../services/pusher/actions';
import {
  setGroupStream,
  setClientMute,
  setClientVolume,
  setStreamingEnabled,
  controlStream,
} from '../../services/snapcast/actions';
import { sortItems, indexToArray } from '../../util/arrays';
import { titleCase } from '../../util/helpers';
import { I18n, i18n } from '../../locale';
import Link from '../Link';
import ErrorBoundary from '../ErrorBoundary';

const Header = ({
  stream,
  server,
}) => {
  const dispatch = useDispatch();
  const {
    id,
    status,
    properties: {
      playbackStatus,
      canPlay,
      canPause,
      canControl,
      metadata: {
        title,
        artist: artists = [],
        artUrl,
      } = {},
    } = {},
  } = stream || {};
  let onClick = null;
  switch (playbackStatus) {
    case 'playing':
      if (canPause) onClick = () => dispatch(controlStream(id, 'pause'));
      break;
    default:
      if (canPlay) onClick = () => dispatch(controlStream(id, 'play'));
      break;
  }

  return (
    <div className="output-control__stream__header">
      <div
        className={[
          'output-control__stream__header__art',
          `output-control__stream__header__art--${onClick !== null ? playbackStatus : 'disabled'}`
        ].join(' ')}
        onClick={onClick}
      >
        <Thumbnail
          image={artUrl}
          size="small"
        />
      </div>
      <div className="output-control__stream__header__content">
        <h5 className="output-control__stream__header__title tooltip">
          {server?.name || id}
          {!canControl && (
            <span className="flag">
              {i18n('snapcast.not_controllable').toUpperCase()}
            </span>
          )}
        </h5>
        {!title && !artists?.length ? (
          <div className="details">
            <I18n path={`common.play_state.${playbackStatus || status}`} />
          </div>
        ) : (
          <ul className="details">
            <li>{title}</li>
            {artists && (
              <li>
                <LinksSentence items={artists.map((name) => ({ name }))} type="artist" nolinks />
              </li>
            )}
          </ul>
        )}
      </div>
    </div>
  );
};

const Group = ({
  setExpanded,
  group: {
    id: groupId,
    name: groupName,
    stream_id,
    clients_ids = [],
  } = {},
}) => {
  const allClients = useSelector((state) => state.snapcast.clients || {});
  const allStreams = indexToArray(useSelector((state) => state.snapcast.streams || {}));
  const clients = clients_ids.length > 0 && Object.keys(allClients).length > 0
    ? clients_ids.map((c) => allClients[c]).filter((c) => c.connected)
    : [];
  const dispatch = useDispatch();

  if (!clients || !clients.length) return null;

  return (
    <div className="output-control__group">
      <h5 className="output-control__group__title">
        <Link
          className="text"
          to={`/settings/services/snapcast/${groupId}`}
          scrollTo="#services-snapcast-groups"
          onClick={() => setExpanded(false)}
        >
          {titleCase(groupName)}
        </Link>
        <DropdownField
          name="Stream"
          value={stream_id}
          icon="settings_input_component"
          options={allStreams.map((s) => ({ value: s.id, label: titleCase(s.id) }))}
          noLabel
          handleChange={
            (value) => dispatch(setGroupStream(groupId, value))
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
                    (value) => dispatch(setClientMute(clientId, value))
                  }
                />
                <VolumeControl
                  volume={volume}
                  mute={mute}
                  onVolumeChange={
                    (value) => dispatch(setClientVolume(clientId, value))
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

const Outputs = ({ setExpanded }) => {
  const dispatch = useDispatch();
  const allGroups = indexToArray(useSelector((state) => state.snapcast.groups || {}));
  const allStreams = useSelector((state) => state.snapcast.streams || {});
  const allServers = indexToArray(useSelector((state) => state.mopidy.servers || {}));
  const groupsByStream = groupBy(allGroups, 'stream_id');
  const { streaming_enabled } = useSelector((state) => state?.snapcast || {});

  return (
    <ErrorBoundary>
      {map(groupsByStream, (groups, id) => {
        const stream = {
          id,
          ...find(allStreams, (s) => s.id === id) || {},
          server: find(allServers, (s) => s.snapcast_stream === id),
          groups,
        };
        return (
          <div className="output-control__stream" key={`stream_${id}`}>
            <Header stream={stream} />
            {
              groups.map(
                (group) => (
                  <Group setExpanded={setExpanded} group={group} key={`group_${group.id}`} />
                )
              )
            }
          </div>
        );
      })}
      <div className="field checkbox" style={{ paddingLeft: 12 }}>
        <label>
          <input
            type="checkbox"
            name="streaming_enabled"
            checked={streaming_enabled}
            onChange={() => dispatch(setStreamingEnabled(!streaming_enabled))}
          />
          <span className="label">
            <I18n path="snapcast.stream_on_this_device" />
          </span>
        </label>
      </div>
    </ErrorBoundary>
  );
}

const Commands = ({ commands }) => {
  const dispatch = useDispatch();

  let items = indexToArray(commands);
  if (items.length <= 0) return null;

  items = sortItems(items, 'sort_order');

  return (
    <ErrorBoundary>
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
    </ErrorBoundary>
  );
};

const OutputControl = ({ force_expanded }) => {
  const snapcastEnabled = useSelector((state) => state.snapcast.enabled);
  const commands = useSelector((state) => state.pusher.commands);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    if (force_expanded && !expanded) {
      setExpanded(true);
    }
  }, [force_expanded]);

  if (!snapcastEnabled && isEmpty(commands)) return null;

  if (expanded) {
    return (
      <span className="output-control">
        {!force_expanded && <div className="click-outside" onClick={() => setExpanded(false)} />}
        <button
          className="control speakers active"
          onClick={() => setExpanded(false)}
        >
          <Icon name="speaker" />
        </button>
        <div className="output-control__inner">
          {!isEmpty(commands) && <Commands commands={commands} />}
          {snapcastEnabled && <Outputs setExpanded={setExpanded} />}
        </div>
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

export default OutputControl;

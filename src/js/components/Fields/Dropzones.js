import React from 'react';
import { useDispatch, useSelector } from 'react-redux';
import { useHistory } from 'react-router-dom';
import { useDragLayer, useDrop } from 'react-dnd';
import * as mopidyActions from '../../services/mopidy/actions';
import Icon from '../Icon';
import { i18n } from '../../locale';
import { encodeUri } from '../../util/format';

const zones = [
  {
    action: 'enqueue',
    title: i18n('actions.add_to_queue'),
    icon: 'play_arrow',
    accept: ['TRACK', 'ALBUM', 'PLAYLIST', 'ARTIST'],
  },
  {
    action: 'enqueue_next',
    title: i18n('actions.play_next'),
    icon: 'play_arrow',
    accept: ['TRACK', 'ALBUM', 'PLAYLIST', 'ARTIST'],
  },
  {
    action: 'add_to_playlist',
    title: i18n('actions.add_to_playlist'),
    icon: 'playlist_add',
    accept: ['TRACK'],
  },
  {
    action: 'create_playlist_and_add',
    title: i18n('modal.edit_playlist.create_playlist'),
    icon: 'playlist_add',
    accept: ['TRACK'],
  },
];

const Dropzones = () => {
  const { isDragging } = useDragLayer((monitor) => ({
    isDragging: monitor.isDragging(),
  }));

  return (
    <div className={`dropzones ${isDragging ? 'dropzones--dragging' : ''}`}>
      {zones.map((zone) => <Dropzone key={zone.action} {...zone} />)}
    </div>
  );
};

const Dropzone = ({
  title,
  icon,
  action,
  accept,
}) => {
  const dispatch = useDispatch();
  const history = useHistory();
  const [{ handlerId, isOver, canDrop }, drop] = useDrop({
    accept,
    collect: (monitor) => {
      return {
        handlerId: monitor.getHandlerId(),
        isOver: monitor.isOver(),
        canDrop: monitor.canDrop(),
      };
    },
    drop: ({ item, items, context }) => {
      const uris = item ? [item.uri] : items.map(({ item: { uri } }) => uri);
      switch (action) {
        case 'enqueue':
          dispatch(mopidyActions.enqueueURIs({ uris, from: context }));
          break;
        case 'enqueue_next':
          dispatch(mopidyActions.enqueueURIs({ uris, from: context, play_next: true }));
          break;
        case 'add_to_playlist':
          history.push(`/modal/add-to-playlist/${encodeUri(uris.join(','))}`);
          break;
        case 'create_playlist_and_add':
          history.push(`/modal/create-playlist/${encodeUri(uris.join(','))}`);
          break;
        default:
          break;
      }
    },
  });

  let className = 'dropzones__item';
  if (isOver && canDrop) className += ' dropzones__item--drag-over';
  if (!canDrop) className += ' dropzones__item--disabled';

  return (
    <div
      ref={drop}
      data-handler-id={handlerId}
      className={className}
    >
      <Icon name={icon} />
      <span className="title">{title}</span>
    </div>
  );
};

export default Dropzones;

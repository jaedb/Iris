import React from 'react';
import { useDispatch, useSelector } from 'react-redux';
import { useHistory } from 'react-router-dom';
import { useDragLayer, useDrop } from 'react-dnd';
import * as mopidyActions from '../../services/mopidy/actions';
import Icon from '../Icon';
import { arrayOf } from '../../util/arrays';
import { i18n } from '../../locale';
import { encodeUri } from '../../util/format';

const zones = [
  {
    action: 'enqueue',
    title: i18n('actions.add_to_queue'),
    icon: 'play_arrow',
  },
  {
    action: 'enqueue_next',
    title: i18n('actions.play_next'),
    icon: 'play_arrow',
  },
  {
    action: 'add_to_playlist',
    title: i18n('actions.add_to_playlist'),
    icon: 'playlist_add',
    accepts: ['tltrack', 'track', 'album', 'playlist', 'artist'],
  },
  {
    action: 'create_playlist_and_add',
    title: i18n('modal.edit_playlist.create_playlist'),
    icon: 'playlist_add',
    accepts: ['tltrack', 'track', 'album', 'playlist', 'artist'],
  },
];

const Dropzones = () => {
  const { isDragging } = useDragLayer((monitor) => ({
    isDragging: monitor.isDragging(),
  }));

  if (!isDragging) return null;

  return (
    <div className="dropzones">
      {zones.map((zone) => <Dropzone key={zone.action} {...zone} />)}
    </div>
  );
};

const Dropzone = ({
  title,
  icon,
  action,
}) => {
  const dispatch = useDispatch();
  const history = useHistory();
  const [{ handlerId, isOver }, drop] = useDrop({
    accept: 'TRACK',
    collect: (monitor) => ({
      handlerId: monitor.getHandlerId(),
      isOver: monitor.isOver(),
    }),
    drop: ({ selected, context }) => {
      const uris = selected.map(({ item: { uri } }) => uri);
      switch (action) {
        case 'enqueue':
          dispatch(mopidyActions.enqueueURIs(uris, context));
          break;
        case 'enqueue_next':
          dispatch(mopidyActions.enqueueURIs(uris, context, true));
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

  return (
    <div
      ref={drop}
      data-handler-id={handlerId}
      className={`dropzones__item ${isOver ? 'dropzones__item--drag-over' : ''}`}
      // onDragEnter={(e) => onDragEnter(e, action)}
      // onDragOver={(e) => onDragOver(e, action)}
      // onDrop={(e) => onDrop(e, action)}
    >
      <Icon name={icon} />
      <span className="title">{title}</span>
    </div>
  );
};

export default Dropzones;

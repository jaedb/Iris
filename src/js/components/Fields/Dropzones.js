import React, { useState } from 'react';
import { useDispatch, useSelector } from 'react-redux';
import { useHistory } from 'react-router-dom';
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
  const dispatch = useDispatch();
  const history = useHistory();
  const [dropTarget, setDropTarget] = useState();
  const {
    victims,
    from_uri,
    active,
  } = useSelector((state) => state.ui.dragger || {});
  if (!active) return null;

  const onDragEnter = (e, action) => {
    e.preventDefault();
    e.stopPropagation();
    setDropTarget(action);
  }

  const onDragOver = (e, action) => {
    e.preventDefault();
    e.stopPropagation();
    setDropTarget(action);
  }

  const onDrop = (e, action) => {
    e.preventDefault();
    e.stopPropagation();
    const uris = arrayOf('uri', victims);
    switch (action) {
      case 'enqueue':
        dispatch(mopidyActions.enqueueURIs(uris, from_uri));
        break;
      case 'enqueue_next':
        dispatch(mopidyActions.enqueueURIs(uris, from_uri, true));
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
  };

  return (
    <div className="dropzones">
      {zones.map(({ title, icon, action }) => (
        <div
          key={action}
          className={`dropzones__item ${dropTarget === action ? ' hover' : ''}`}
          onDragEnter={(e) => onDragEnter(e, action)}
          onDragOver={(e) => onDragOver(e, action)}
          onDrop={(e) => onDrop(e, action)}
        >
          <Icon name={icon} />
          <span className="title">{title}</span>
        </div>
      ))}
    </div>
  );
};

export default Dropzones;

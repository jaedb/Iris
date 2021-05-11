import React from 'react';
import { useDispatch, useSelector } from 'react-redux';
import { useHistory } from 'react-router-dom';
import Dropzone from './Dropzone';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import { arrayOf } from '../../util/arrays';
import { i18n } from '../../locale';
import { encodeUri } from '../../util/format';

const Dropzones = () => {
  const dispatch = useDispatch();
  const history = useHistory();
  const {
    victims,
    from_uri,
    active,
  } = useSelector((state) => state.ui.dragger || {});
  const zones = [
    {
      title: i18n('actions.add_to_queue'),
      icon: 'play_arrow',
      action: 'enqueue',
    },
    {
      title: i18n('actions.play_next'),
      icon: 'play_arrow',
      action: 'enqueue_next',
    },
    {
      title: i18n('actions.add_to_playlist'),
      icon: 'playlist_add',
      action: 'add_to_playlist',
      accepts: ['tltrack', 'track', 'album', 'playlist', 'artist'],
    },
    {
      title: i18n('modal.edit_playlist.create_playlist'),
      icon: 'playlist_add',
      action: 'create_playlist_and_add',
      accepts: ['tltrack', 'track', 'album', 'playlist', 'artist'],
    },
  ];

  if (!active) return null;

  const handleMouseUp = (zone) => {
    const uris = arrayOf('uri', victims);
    switch (zone.action) {
      case 'enqueue':
        dispatch(mopidyActions.enqueueURIs(uris, from_uri));
        break;
      case 'enqueue_next':
        dispatch(mopidyActions.enqueueURIs(uris, from_uri, true));
        break;
      case 'add_to_playlist':
        history.push(`/add-to-playlist/${encodeUri(uris.join(','))}`);
        break;
      case 'create_playlist_and_add':
        history.push(`/playlist/create/${encodeUri(uris.join(','))}`);
        break;
      default:
        break;
    }
  };

  return (
    <div className="dropzones">
      {
        zones.map((zone) => (
          <Dropzone
            key={zone.action}
            data={zone}
            handleMouseUp={handleMouseUp}
          />
        ))
      }
    </div>
  );
};

export default Dropzones;

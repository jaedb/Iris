import React from 'react';
import { useDispatch } from 'react-redux';
import { useHistory } from 'react-router-dom';
import { I18n } from '../../locale';
import Link from '../Link';
import {
  loadAlbum,
  loadArtist,
  loadTrack,
  deletePlaylist,
} from '../../services/core/actions';
import {
  hideContextMenu,
  createNotification,
} from '../../services/ui/actions';
import {
  following,
} from '../../services/spotify/actions';
import {
  startRadio,
} from '../../services/pusher/actions';
import {
  titleCase,
} from '../../util/helpers';
import { arrayOf } from '../../util/arrays';
import { encodeUri } from '../../util/format';
import { enqueueURIs, playURIs } from '../../services/mopidy/actions';

const ContextMenuItems = ({ context_menu }) => {
  const {
    type,
    item,
    items,
    context,
  } = context_menu;

  switch (type) {
    case 'album': {
      return (
        <>
          <Play uris={[item.uri]} context={context} />
          <Enqueue uris={[item.uri]} context={context} next />
          <Enqueue uris={[item.uri]} context={context} />
          <Divider />
          {item.provider === 'spotify' && (
            <>
              <Library uri={item.uri} inLibrary={item.in_library} />
              <Divider />
            </>
          )}
          {item.artists?.length > 0 && <GoTo type="artist" uri={item.artists[0].uri} />}
          <Copy uris={[item.uri]} />
          <Refresh uri={item.uri} action={loadAlbum} />
        </>
      );
    }
    case 'artist': {
      const urisToPlay = item.tracks ? arrayOf('uri', item.tracks) : [item.uri];
      return (
        <>
          <Play uris={urisToPlay} context={context} />
          <Enqueue uris={urisToPlay} context={context} next />
          <Enqueue uris={urisToPlay} context={context} />
          {item.provider === 'spotify' && (
            <>
              <Radio uris={[item.uri]} />
              <Divider />
              <Library uri={item.uri} inLibrary={item.in_library} />
              <Divider />
              <Discover uris={[item.uri]} context={context} />
              <Divider />
            </>
          )}
          <Copy uris={[item.uri]} />
          <Refresh uri={item.uri} action={loadArtist} />
        </>
      );
    }
    case 'playlist': {
      return (
        <>
          <Play uris={[item.uri]} context={context} />
          <Enqueue uris={[item.uri]} context={context} next />
          <Enqueue uris={[item.uri]} context={context} />
          <Divider />
          {item.user && <GoTo type="user" uri={item.user.uri} />}
          <Copy uris={[item.uri]} />
          {item.can_edit && (
            <>
              <Edit uri={item.uri} type="playlist" />
              <Delete uri={item.uri} action={deletePlaylist} />
            </>
          )}
          <Refresh uri={item.uri} action={loadTrack} />
        </>
      );
    }
    case 'track': {
      return (
        <>
          <Play uris={[item.uri]} context={context} />
          <Enqueue uris={[item.uri]} context={context} next />
          <Enqueue uris={[item.uri]} context={context} />
          <Divider />
          {item.provider === 'spotify' && (
            <>
              <Discover uris={[item.uri]} context={context} />
              <Divider />
            </>
          )}
          <Copy uris={[item.uri]} />
          <Refresh uri={item.uri} action={loadTrack} />
        </>
      );
    }
    default: {
      return (
        <>
          <Play uris={arrayOf('uri', items)} context={context} />
          <Enqueue uris={arrayOf('uri', items)} context={context} next />
          <Enqueue uris={arrayOf('uri', items)} context={context} />
          <Divider />
          <Copy uris={arrayOf('uri', items)} />
        </>
      );
    }
  }
};

const Divider = () => <div className="context-menu__divider" />;

const Refresh = ({
  uri,
  action,
}) => {
  const dispatch = useDispatch();
  const onClick = () => {
    dispatch(action(uri, { forceRefetch: true, full: true }));
    dispatch(hideContextMenu());
  };
  return (
    <div className="context-menu__item">
      <a className="context-menu__item__link" onClick={onClick}>
        <span className="context-menu__item__label">
          <I18n path="context_menu.refresh" />
        </span>
      </a>
    </div>
  );
};

const Discover = ({
  uris,
}) => {
  const history = useHistory();
  const dispatch = useDispatch();
  const onClick = () => {
    dispatch(hideContextMenu());
    const discoverUri = encodeUri(`iris:discover:${uris.map((uri) => encodeUri(uri)).join(',')}`);
    history.push(`/discover/recommendations/${discoverUri}`);
  };
  return (
    <div className="context-menu__item">
      <a className="context-menu__item__link" onClick={onClick}>
        <span className="context-menu__item__label">
          <I18n path="context_menu.discover_similar" />
        </span>
      </a>
    </div>
  );
};

const Library = ({
  uri,
  inLibrary,
}) => {
  const dispatch = useDispatch();
  const onClick = () => {
    dispatch(hideContextMenu());
    dispatch(following(uri, inLibrary ? 'DELETE' : 'PUT'));
  };
  return (
    <div className="context-menu__item">
      <a className="context-menu__item__link" onClick={onClick}>
        <span className="context-menu__item__label">
          <I18n path={`actions.${inLibrary ? 'remove_from' : 'add_to'}_library`} />
        </span>
      </a>
    </div>
  );
};

const GoTo = ({
  type,
  uri,
}) => {
  const history = useHistory();
  const dispatch = useDispatch();
  const onClick = () => {
    dispatch(hideContextMenu());
    console.debug({uri})
    history.push(`/${type}/${encodeUri(uri)}`);
  };
  return (
    <div className="context-menu__item">
      <a className="context-menu__item__link" onClick={onClick}>
        <span className="context-menu__item__label">
          <I18n path={`context_menu.go_to_${type}`} />
        </span>
      </a>
    </div>
  );
};

const Edit = ({
  uri,
  type,
}) => {
  return (
    <div className="context-menu__item">
      <Link
        className="context-menu__item__link"
        to={`/modal/edit-${type}/${encodeUri(uri)}`}
      >
        <span className="context-menu__item__label">
          <I18n path="actions.edit" />
        </span>
      </Link>
    </div>
  );
};

const Delete = ({
  uri,
  action,
}) => {
  const dispatch = useDispatch();
  const onClick = () => {
    dispatch(hideContextMenu());
    dispatch(action(uri));
  };
  return (
    <div className="context-menu__item">
      <a className="context-menu__item__link" onClick={onClick}>
        <span className="context-menu__item__label">
          <I18n path="actions.delete" />
        </span>
      </a>
    </div>
  );
};

const Play = ({
  uris,
  context,
}) => {
  const dispatch = useDispatch();
  const onClick = () => {
    dispatch(playURIs(uris, context));
    dispatch(hideContextMenu());
  };
  return (
    <div className="context-menu__item">
      <a className="context-menu__item__link" onClick={onClick}>
        <span className="context-menu__item__label">
          <I18n path="actions.play" />
        </span>
      </a>
    </div>
  );
};

const Enqueue = ({
  uris,
  next,
  context,
}) => {
  const dispatch = useDispatch();
  const onClick = () => {
    dispatch(enqueueURIs(uris, context, next));
    dispatch(hideContextMenu());
  };
  return (
    <div className="context-menu__item">
      <a className="context-menu__item__link" onClick={onClick}>
        <span className="context-menu__item__label">
          <I18n path={next ? 'context_menu.play_next' : 'actions.add_to_queue'} />
        </span>
      </a>
    </div>
  );
};

const Radio = ({
  uris,
}) => {
  const dispatch = useDispatch();
  const onClick = () => {
    dispatch(hideContextMenu());
    dispatch(startRadio(uris))
  };
  return (
    <div className="context-menu__item">
      <a className="context-menu__item__link" onClick={onClick}>
        <span className="context-menu__item__label">
          <I18n path="context_menu.start_radio" />
        </span>
      </a>
    </div>
  );
};

const Copy = ({
  uris,
}) => {
  const dispatch = useDispatch();
  const onClick = () => {
    const temp = $('<input>');
    $('body').append(temp);
    temp.val(uris.join(',')).select();
    document.execCommand('copy');
    temp.remove();

    dispatch(createNotification({ content: `Copied ${uris.length} URIs` }));
    dispatch(hideContextMenu());
  };
  return (
    <div className="context-menu__item">
      <a className="context-menu__item__link" onClick={onClick}>
        <span className="context-menu__item__label">
          <I18n path="context_menu.copy_uri" />
        </span>
      </a>
    </div>
  );
};

export default {
  ContextMenuItems,
};

export {
  ContextMenuItems,
};

import React, { useEffect } from 'react';
import { useDispatch } from 'react-redux';
import { compact } from 'lodash';
import { I18n } from '../../locale';
import Link from '../Link';
import Icon from '../Icon';
import { encodeUri } from '../../util/format';
import {
  arrayOf,
  sortItems,
} from '../../util/arrays';
import {
  loadLibrary,
  addTracksToPlaylist,
} from '../../services/core/actions';
import {
  hideContextMenu,
} from '../../services/ui/actions';

export default ({
  context_menu: {
    item,
    items = [],
  },
  onClose,
  allPlaylists = [],
  loading,
  providers,
}) => {
  const dispatch = useDispatch();

  useEffect(() => {
    if (!loading && !allPlaylists?.length) {
      providers.forEach((provider) => dispatch(loadLibrary(provider.uri, 'playlists')));
    }
  }, []);

  const uris = item ? [item.uri] : arrayOf('uri', items);
  const encodedUris = uris && uris.length > 0 ? encodeUri(uris.join(',')) : '';

  const onClick = (playlist_uri) => {
    dispatch(hideContextMenu());
    dispatch(addTracksToPlaylist(playlist_uri, uris));
  };

  let playlists = compact(allPlaylists.map((playlist) => {
    if (!playlist.can_edit) return null;
    return {
      ...playlist,
      is_pinned: false //TODO,
    };
  }));
  playlists = sortItems(playlists, 'name');
  playlists = sortItems(playlists, 'is_pinned', true);

  return (
    <div className="context-menu__section context-menu__section--submenu">
      <div className="context-menu__item context-menu__item--functional">
        <a
          className="context-menu__item__link"
          onClick={onClose}
        >
          <span className="context-menu__item__label">
            <Icon name="arrow_back" />
            <span>
              <I18n path="actions.back" />
            </span>
          </span>
        </a>
      </div>
      <div className="context-menu__item context-menu__item--functional">
        <Link className="context-menu__item__link" to={`/modal/create-playlist/${encodedUris}`}>
          <span className="context-menu__item__label">
            <Icon name="add" />
            <span>
              <I18n path="context_menu.add_to_playlist.new_playlist" />
            </span>
          </span>
        </Link>
      </div>
      {loading ? (
        <div className="context-menu__item context-menu__item--loader">
          <Loader className="context-menu__item" mini loading />
        </div>
      ) : (
        <>
          {playlists.length > 0 ? playlists.map((playlist) => {
            const ElementTag = playlist.is_pinned ? 'em' : 'span'; // To css-target last-child
            return (
              <ElementTag className="context-menu__item" key={playlist.uri}>
                <a
                  className="context-menu__item__link"
                  onClick={() => onClick(playlist.uri)}
                >
                  <span className="context-menu__item__label">
                    {playlist.name}
                  </span>
                </a>
              </ElementTag>
            );
          }) : (
            <span className="context-menu__item">
              <span className="context-menu__item mid_grey-text">
                <span className="context-menu__item__link context-menu__item__link--inactive">
                  <I18n path="context_menu.add_to_playlist.no_playlists" />
                </span>
              </span>
            </span>
          )}
        </>
      )}
    </div>
  );
};

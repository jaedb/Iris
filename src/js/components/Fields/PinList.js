import React, { useEffect } from 'react';
import { useSelector, useDispatch } from 'react-redux';
import Link from '../Link';
import * as coreActions from '../../services/core/actions';

const PinListItem = ({ uri }) => {
  const playlists = useSelector((state) => state.core.playlists);
  const dispatch = useDispatch();

  useEffect(() => {
    dispatch(coreActions.loadItem(uri));
  }, [uri]);

  const playlist = playlists[uri] || { uri };
  return (
    <Link
      to={`/playlist/${uri}`}
      className="sidebar__menu__item sidebar__menu__item--submenu"
      activeClassName="sidebar__menu__item--active"
    >
      {playlist.name || playlist.uri}
    </Link>
  );
};

const PinList = () => {
  const uris = useSelector((state) => state.core.pinned || []);

  if (uris.length <= 0) return null;

  return (
    <div>
      {
        uris.map((uri) => <PinListItem uri={uri} key={uri} />)
      }
    </div>
  );
};

export default PinList;

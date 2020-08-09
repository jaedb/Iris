import React from 'react';
import { useSelector } from 'react-redux';
import Link from '../Link';

const PinList = () => {
  const uris = useSelector((state) => state.core.pinned || []);
  const playlists = useSelector((state) => state.core.playlists);

  if (uris.length <= 0) return null;

  return (
    <div>
      {
        uris.map((uri) => {
          const playlist = playlists[uri] || { uri };
          return (
            <Link
              to={`/playlist/${uri}`}
              className="sidebar__menu__item sidebar__menu__item--submenu"
              activeClassName="sidebar__menu__item--active"
              key={uri}
            >
              {playlist.name || playlist.uri}
            </Link>
          );
        })
      }
    </div>
  );
};

export default PinList;

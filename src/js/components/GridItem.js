import React, { useEffect } from 'react';
import { useDispatch, useSelector } from 'react-redux';
import { useDrag } from 'react-dnd';
import Link from './Link';
import { SourceIcon } from './Icon';
import Thumbnail from './Thumbnail';
import LinksSentence from './LinksSentence';
import { I18n } from '../locale';
import { encodeUri } from '../util/format';
import { isTouchDevice } from '../util/helpers';

import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as spotifyActions from '../services/spotify/actions';

const SecondaryLine = ({
  sourceIcon = true,
  item: {
    uri,
    type,
    tracks_total,
    tracks = [],
    followers,
    albums_uris = [],
    artists,
  } = {},
}) => {
  let trackCount = 0;
  if (tracks) trackCount = tracks.length;
  if (tracks_total) trackCount = tracks_total;

  const items = () => {
    switch (type) {
      case 'mood':
      case 'directory':
      case 'playlist_group':
        return null;
      case 'playlist':
        return <li><I18n path="specs.tracks" count={trackCount} /></li>;
      case 'artist':
        return (
          <>
            {followers > 0 && <li><I18n path="specs.followers" count={followers.toLocaleString()} /></li>}
            {albums_uris.length > 0 && <li><I18n path="specs.albums" count={albums_uris.length} /></li>}
          </>
        );
      case 'album':
        return <li>{artists && <LinksSentence nolinks items={artists} type="artist" />}</li>;
      default:
        return (
          <>
            {artists && <li><LinksSentence nolinks items={artists} type="artist" /></li> }
            {followers && <li><I18n path="specs.followers" count={followers.toLocaleString()} /></li>}
          </>
        );
    }
  };

  return (
    <ul className="grid__item__secondary__content details">
      {sourceIcon && <SourceIcon uri={uri} />}
      {items()}
    </ul>
  );
};

const GridItem = ({
  item: itemProp,
  getLink,
  sourceIcon,
}) => {
  let item = itemProp;
  if (item.album) item = { ...item, ...item.album };

  const dispatch = useDispatch();
  const grid_glow_enabled = useSelector((state) => state.ui.grid_glow_enabled);
  const spotify_available = useSelector((state) => state.spotify.access_token);
  const [_, drag] = useDrag({
    type: item?.type?.toUpperCase() || 'UNKNOWN',
    item: { item, context: item },
  });
  const tile = ['playlist_group', 'mood', 'category'].indexOf(item?.type) > -1

  const onContextMenu = (e) => {
    e.preventDefault();
    dispatch(
      uiActions.showContextMenu({
        e,
        type: item.type,
        item,
      }),
    );
  };

  // Load images
  useEffect(() => {
    if (!item.images && !item.loading) {
      switch (item.type) {
        case 'artist':
          if (spotify_available) {
            dispatch(spotifyActions.getArtistImages(item));
          }
          break;
        case 'playlist':
        case 'album':
          dispatch(mopidyActions.getImages([item.uri]));
          break;
        default:
          break;
      }
    }
  }, [item.images]);

  // Build link
  let to = '';
  const getLinkResult = getLink ? getLink(item) : undefined;
  if (getLinkResult) {
    to = getLinkResult;
  } else if (item.link) {
    to = item.link;
  } else {
    to = `/${item.type}/${encodeUri(item.uri)}`;
    if (item.name && item.type !== 'artist') {
      // Strip out "%"; this causes conflicts with our uri decoder
      to += `/${encodeURIComponent(item.name.replace('%', '').replace('/', ''))}`;
    }
  }

  return (
    <div
      ref={isTouchDevice() ? undefined : drag}
      className={`grid__item grid__item--${itemProp.type} ${tile ? 'grid__item--tile' : ''}`}
    >
      <Link
        to={to}
        onContextMenu={onContextMenu}
      >
        <Thumbnail
          glow={grid_glow_enabled}
          size="medium"
          className="grid__item__thumbnail"
          images={item.images || item.icons}
          type={item.type}
          loading={item.loading}
        />
        <div className="grid__item__name">
          {item.name
            ? <span title={item.name}>{item.name}</span>
            : <span className="opaque-text">{item.uri}</span>
          }
        </div>
        <div className="grid__item__secondary">
          <SecondaryLine item={item} sourceIcon={sourceIcon} />
        </div>
      </Link>
    </div>
  );
};

export {
  GridItem,
};

export default {
  GridItem,
};

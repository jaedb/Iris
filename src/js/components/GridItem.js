import React, { useEffect } from 'react';
import { useDispatch, useSelector } from 'react-redux';
import Link from './Link';
import { SourceIcon } from './Icon';
import Thumbnail from './Thumbnail';
import LinksSentence from './LinksSentence';
import { I18n } from '../locale';
import { encodeUri } from '../util/format';

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

  const onContextMenu = (e) => {
    e.preventDefault();
    dispatch(
      uiActions.showContextMenu({
        e,
        context: item.type,
        uris: [item.uri],
        items: [item],
        tracklist_uri: item.uri, // not needed?
      }),
    );
  };

  // Load images
  useEffect(() => {
    if (!item.images) {
      switch (item.type) {
        case 'artist':
          if (spotify_available) {
            dispatch(spotifyActions.getArtistImages(item));
          }
          break;
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
  }

  return (
    <Link
      to={to}
      onContextMenu={onContextMenu}
      className={`grid__item grid__item--${itemProp.type}`}
    >
      <Thumbnail
        glow={grid_glow_enabled}
        size="medium"
        className="grid__item__thumbnail"
        images={item.images || item.icons}
        type={item.type}
      />
      <div className="grid__item__name">
        {item.name ? item.name : <span className="opaque-text">{item.uri}</span>}
      </div>
      <div className="grid__item__secondary">
        <SecondaryLine item={item} sourceIcon={sourceIcon} />
      </div>
    </Link>
  );
};

export {
  GridItem,
};

export default {
  GridItem,
};

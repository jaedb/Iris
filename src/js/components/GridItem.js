import React from 'react';
import handleViewport from 'react-in-viewport';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { scrollTo, sourceIcon } from '../util/helpers';
import Link from './Link';
import Icon from './Icon';
import Thumbnail from './Thumbnail';
import LinksSentence from './LinksSentence';
import { I18n } from '../locale';
import { encodeUri } from '../util/format';

import * as uiActions from '../services/ui/actions';
import * as lastfmActions from '../services/lastfm/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as spotifyActions from '../services/spotify/actions';

const SecondaryLine = ({
  item: {
    type,
    tracks_total,
    tracks = [],
    followers,
    albums_uris,
    artists,
  } = {},
}) => {
  let trackCount = 0;
  if (tracks) trackCount = tracks.length;
  if (tracks_total) trackCount = tracks_total;

  switch (type) {
    case 'playlist':
      return (
        <span className="grid__item__secondary__content">
          <I18n path="specs.tracks" count={trackCount} />
        </span>
      );

    case 'artist':
      return (
        <span className="grid__item__secondary__content">
          {followers && <I18n path="specs.followers" count={followers.toLocaleString()} />}
          {albums_uris && <I18n path="specs.albums" count={albums_uris.length} />}
        </span>
      );

    case 'album':
      return (
        <span className="grid__item__secondary__content">
          {artists && <LinksSentence nolinks items={artists} type="artist" />}
        </span>
      );

    default:
      return (
        <span className="grid__item__secondary__content">
          {artists && <LinksSentence nolinks items={artists} type="artist" /> }
          {followers && <I18n path="specs.followers" count={followers.toLocaleString()} />}
        </span>
      );
  }
};

const GridItemComponent = ({
  item: itemProp,
  itemHeight,
  getLink,
  show_source_icon,
  grid_glow_enabled,
  spotify_available,
  isFirst,
  uiActions: {
    showContextMenu,
  },
  mopidyActions: {
    getImages: getMopidyImages,
  },
  spotifyActions: {
    getArtistImages: getSpotifyImages,
  },
  inViewport,
  forwardedRef,
  setItemHeight,
}) => {
  if (!itemProp) return null;
  let item = itemProp;
  if (item.album) item = { ...item, ...item.album };

  // Listen for changes to our height, and pass it up to our Grid. This is then used to build the
  // placeholder elements when out of viewport. We only care about the first item because this
  // represents the same heights for everything else (in almost all circumstances).
  if (isFirst && forwardedRef.current) {
    const { current: { clientHeight } } = forwardedRef;
    if (clientHeight !== itemHeight) {
      setItemHeight(clientHeight);
    }
  }

  const onContextMenu = (e) => {
    e.preventDefault();
    showContextMenu({
      e,
      context: item.type,
      uris: [item.uri],
      items: [item],
      tracklist_uri: item.uri, // not needed?
    });
  };

  // Load images
  if (!item.images && inViewport) {
    switch (item.type) {
      case 'artist':
        if (spotify_available) getSpotifyImages(item);
        break;
      case 'album':
        getMopidyImages([item.uri]);
        break;
      default:
        break;
    }
  }

  // Build link
  let to = '';
  if (getLink) {
    to = getLink(item);
  } else if (item.link) {
    to = item.link;
  } else {
    to = `/${item.type}/${encodeUri(item.uri)}`;
  }

  return (
    <span className={`grid__item grid__item--${item.type}`} ref={forwardedRef}>
      {inViewport || isFirst ? (
        <Link
          to={to}
          onClick={scrollTo}
          onContextMenu={onContextMenu}
          className="grid__item__inner"
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
            {show_source_icon && (
              <Icon name={sourceIcon(item.uri)} type="fontawesome" className="source" />
            )}
            <SecondaryLine item={item} />
          </div>
        </Link>
      ) : (
        <div style={{ height: itemHeight }} />
      )}
    </span>
  );
};

const mapStateToProps = (state) => {
  const {
    ui: {
      grid_glow_enabled,
    },
    spotify: {
      access_token: spotify_available,
    },
  } = state;

  return {
    grid_glow_enabled,
    spotify_available,
  };
};

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

const GridItem = connect(mapStateToProps, mapDispatchToProps)(handleViewport(GridItemComponent));

export {
  GridItem,
};

export default {
  GridItem,
};

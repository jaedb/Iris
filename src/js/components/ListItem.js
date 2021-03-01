import React from 'react';
import handleViewport from 'react-in-viewport';
import { useDispatch, useSelector } from 'react-redux';
import LinksSentence from './LinksSentence';
import { dater } from './Dater';
import { nice_number } from './NiceNumber';
import URILink from './URILink';
import ContextMenuTrigger from './ContextMenuTrigger';
import Icon from './Icon';
import Thumbnail from './Thumbnail';
import Popularity from './Popularity';
import Link from './Link';
import { I18n } from '../locale';
import { encodeUri } from '../util/format';

import * as uiActions from '../services/ui/actions';
import * as lastfmActions from '../services/lastfm/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as spotifyActions from '../services/spotify/actions';

const getValue = (item = {}, name = '') => {
  const { [name]: value } = item;

  switch (name) {
    case 'tracks': {
      const {
        tracks_total: total,
      } = item;
      if (!total && !value) return null;
      return <I18n path="specs.tracks" count={nice_number(total || value.length)} />;
    }
    case 'artists': {
      const {
        artists_total: total,
        artists_uris: array,
      } = item;
      if (Array.isArray(value)) return <LinksSentence items={value} type="artist" />;
      if (!total && !array) return null;
      return <I18n path="specs.artists" count={nice_number(total || array.length)} />;
    }
    case 'albums': {
      const {
        albums_total: total,
        albums_uris: array,
      } = item;
      if (!total && !array) return null;
      return <I18n path="specs.albums" count={total || array.length} />;
    }
    default:
      break;
  }

  // All options beyond here don't play well with null/undefined
  if (!value) return null;

  switch (name) {
    case 'followers':
      return <I18n path="specs.followers" count={nice_number(value)} />;
    case 'listeners':
      return <I18n path="specs.listeners" count={nice_number(value)} />;
    case 'added_at':
      return <I18n path="specs.added_ago" time={dater('ago', value)} />;
    case 'last_modified':
      return <I18n path="specs.updated_ago" time={dater('ago', value)} />;
    case 'owner':
      return <URILink type="user" uri={value.uri}>{value.id}</URILink>;
    case 'popularity':
      return <Popularity full popularity={value} />;
    default:
      break;
  }

  if (value === true) return <Icon name="check" />;
  if (typeof (value) === 'number') return <span>{value.toLocaleString()}</span>;
  return value;
};

const ListItemComponent = ({
  item,
  middle_column,
  right_column,
  thumbnail,
  details,
  nocontext,
  forwardedRef,
  getLink,
  isFirst,
  inViewport,
  itemHeight,
  setItemHeight,
}) => {
  if (!item) return null;

  // Listen for changes to our height, and pass it up to our Grid. This is then used to build the
  // placeholder elements when out of viewport. We only care about the first item because this
  // represents the same heights for everything else (in almost all circumstances).
  if (isFirst && forwardedRef.current) {
    const { current: { clientHeight } } = forwardedRef;
    if (clientHeight !== itemHeight) {
      console.debug({ clientHeight, itemHeight })
      setItemHeight(clientHeight);
    }
  }

  let class_name = 'list__item';
  if (item.type) class_name += ` list__item--${item.type}`;
  if (item.loading) class_name += ' list__item--loading';
  if (middle_column) class_name += ' list__item--has-middle-column';
  if (thumbnail) class_name += ' list__item--has-thumbnail';
  if (details) class_name += ' list__item--has-details';

  // Return our placeholder as soon as possible, avoiding additional hooks and code
  if (!inViewport && !isFirst) {
    return (
      <div className={class_name} ref={forwardedRef}>
        <div style={{ height: itemHeight }} />
      </div>
    );
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

  const dispatch = useDispatch();
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
  const grid_glow_enabled = useSelector((state) => state.ui.grid_glow_enabled);
  const spotify_available = useSelector((state) => state.spotify.access_token);

  return (
    <div className={class_name} onContextMenu={onContextMenu} ref={forwardedRef}>
      <Link to={to} className="list__item__inner">
        {
          right_column && !nocontext && (
            <div className="list__item__column list__item__column--right">
              {
                right_column.map((column) => (
                  <span
                    className={`list__item__column__item list__item__column__item--${column.replace('.', '_')}`}
                    key={`${column}`}
                  >
                    {getValue(item, column)}
                  </span>
                ))
              }
              {!nocontext && (
                <ContextMenuTrigger
                  className="list__item__column__item list__item__column__item--context-menu-trigger subtle"
                  onTrigger={onContextMenu}
                />
              )}
            </div>
          )
        }
        <div className="list__item__column list__item__column--name">
          {thumbnail && (
            <Thumbnail
              className="list__item__column__item list__item__column__item--thumbnail"
              images={item.images}
              size="small"
            />
          )}
          <div className="list__item__column__item list__item__column__item--name">
            {
              item.name !== undefined
                ? getValue(item, 'name')
                : <span className="grey-text">{item.uri}</span>
            }
          </div>

          {details ? (
            <ul className="list__item__column__item list__item__column__item--details details">
              {
                details.map((detail) => {
                  const value = getValue(item, detail);
                  if (!value) return null;
                  return (
                    <li
                      className={`details__item details__item--${detail.replace('.', '_')}`}
                      key={detail}
                    >
                      {value}
                    </li>
                  );
                })
              }
            </ul>
          ) : null}
        </div>

        {middle_column && (
          <div className="list__item__column list__item__column--middle">
            {
              middle_column.map((column) => (
                <span
                  className={`list__item__column__item list__item__column__item--${column.replace('.', '_')}`}
                  key={column}
                >
                  {getValue(item, column)}
                </span>
              ))
            }
          </div>
        )}
      </Link>
    </div>
  );
};

const ListItem = handleViewport(ListItemComponent);

export {
  ListItem,
};

export default {
  ListItem,
};

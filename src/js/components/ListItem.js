import React, { useEffect } from 'react';
import { useDispatch, useSelector } from 'react-redux';
import { useHistory, useLocation } from 'react-router-dom';
import LinksSentence from './LinksSentence';
import { dater } from './Dater';
import { nice_number } from './NiceNumber';
import URILink from './URILink';
import ContextMenuTrigger from './ContextMenuTrigger';
import Icon from './Icon';
import Thumbnail from './Thumbnail';
import Popularity from './Popularity';
import { I18n } from '../locale';
import { encodeUri } from '../util/format';
import { sourceIcon } from '../util/helpers';
import { updateScrollPosition } from './Link';

import * as uiActions from '../services/ui/actions';
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
    case 'source':
      return <Icon type="fontawesome" name={sourceIcon(item.uri)} fixedWidth />;
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

const ListItem = ({
  item,
  middle_column,
  right_column,
  details,
  thumbnail,
  nocontext,
  getLink,
}) => {
  if (!item) return null;

  const dispatch = useDispatch();
  const spotify_available = useSelector((state) => state.spotify.access_token);
  const history = useHistory();
  const location = useLocation();

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

  /**
   * Handled as a click, rather than an object so we can direct to a link, UNLESS we have clicked
   * on a sub-link (we may have nested links to Albums for example). HTML does not allow nested
   * <a> tags.
   *
   * @param {Object} e
   */
  const onClick = (e) => {
    let to = '';
    if (getLink) {
      to = getLink(item);
    } else if (item.link) {
      to = item.link;
    } else {
      to = `/${item.type}/${encodeUri(item.uri)}`;
    }

    if (e.target.tagName.toLowerCase() !== 'a') {
      updateScrollPosition({ location, history });
      e.preventDefault();
      history.push(to);
    }
  };

  let className = 'list__item';
  if (item.type) className += ` list__item--${item.type}`;
  if (item.loading) className += ' list__item--loading';
  if (middle_column) className += ' list__item--has-middle-column';
  if (thumbnail) className += ' list__item--has-thumbnail';
  if (details) className += ' list__item--has-details';

  return (
    <div
      className={className}
      onContextMenu={onContextMenu}
      onClick={onClick}
    >
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
    </div>
  );
};

export {
  ListItem,
};

export default {
  ListItem,
};

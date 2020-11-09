
import React from 'react';

import LinksSentence from './LinksSentence';
import { Dater, dater } from './Dater';
import URILink from './URILink';
import ContextMenuTrigger from './ContextMenuTrigger';
import Icon from './Icon';
import Thumbnail from './Thumbnail';
import Popularity from './Popularity';
import {
  uriType,
  scrollTo,
} from '../util/helpers';
import { I18n } from '../locale';
import { arrayOf } from '../util/arrays';

export default class ListItem extends React.Component {
  componentDidMount() {
    const { item, mopidyActions, discogsActions } = this.props;
    if (!item) return;

    // If the item that has just been mounted doesn't have images,
    // try fetching them from LastFM
    if (!item.images) {
      switch (uriType(item.uri)) {
        case 'artist':
          if (discogsActions) {
            //discogsActions.getArtistImages(item.uri, item);
          }
          break;

        case 'album':
          if (mopidyActions) {
            mopidyActions.getImages([item.uri]);
          }
          break;
      }
    }
  }

  handleClick = (e) => {
    const {
      history,
      link_prefix = '',
      item: {
        uri,
      },
    } = this.props;

    if (e.target.tagName.toLowerCase() !== 'a') {
      e.preventDefault();
      history.push(`${link_prefix}${encodeURIComponent(uri)}`);
      scrollTo();
    }
  }

  handleMouseDown = (e) => {
    const {
      history,
      link_prefix = '',
      item: {
        uri,
      },
    } = this.props;

    if (e.target.tagName.toLowerCase() !== 'a') {
      e.preventDefault();
      history.push(`${link_prefix}${encodeURIComponent(uri)}`);
      scrollTo();
    }
  }

  handleContextMenu = (e) => {
    const {
      handleContextMenu,
      item,
    } = this.props;

    if (handleContextMenu) {
      e.preventDefault();
      handleContextMenu(e, item);
    }
  }

  /**
   * TODO
   * 
   * THIS WHOLE BLOCK NEEDS A REVISIT.
   * Surely there is a cleaner way to pull values? Perhaps simplify it like our new sortItems?
   */
  renderValue = (key) => {
    const {
      item: {
        [key]: value,
        ...item
      } = {},
    } = this.props;

    if (key === 'tracks') {
      const {
        tracks_total: total,
        tracks: array,
      } = item;
      if (!total && !array) return null;
      return <I18n path="specs.tracks" count={total || array.length} />;
    }
    if (key === 'artists') {
      const {
        artists_total: total,
        artists_uris: array,
      } = item;
      if (!total && !array) return null;
      return <I18n path="specs.artists" count={total || array.length} />;
    }
    if (key === 'albums') {
      const {
        albums_total: total,
        albums_uris: array,
      } = item;
      if (!total && !array) return null;
      return <I18n path="specs.albums" count={total || array.length} />;
    }

    // All options beyond here don't play well with null/undefined
    if (!value) return null;

    if (key === 'followers') {
      return <I18n path="specs.followers" count={value.toLocaleString()} />;
    }
    if (key === 'added_at') {
      return <I18n path="specs.added_ago" time={dater('ago', value)} />;
    }
    if (key === 'last_modified') {
      return <I18n path="specs.updated_ago" time={dater('ago', value)} />;
    }
    if (key === 'owner') {
      return <URILink type="user" uri={value.uri}>{value.id}</URILink>;
    }
    if (key === 'popularity') {
      return <Popularity full popularity={value} />;
    }

    if (value === true) return <Icon name="check" />;
    if (typeof (value) === 'number') return <span>{value.toLocaleString()}</span>;
    return value;
  }

  render() {
    const {
      item,
      middle_column,
      right_column,
      thumbnail,
      details,
      nocontext,
    } = this.props;
    if (!item) {
      return null;
    }

    let class_name = 'list__item';
    if (item.type) class_name += ` list__item--${item.type}`;
    if (middle_column) class_name += ' list__item--has-middle-column';
    if (thumbnail) class_name += ' list__item--has-thumbnail';
    if (details) class_name += ' list__item--has-details';

    return (
      <div
        className={class_name}
        onClick={this.handleClick}
        onContextMenu={this.handleContextMenu}
      >
        {
          right_column && !nocontext && (
            <div className="list__item__column list__item__column--right">
              {
								right_column.map((column, index) => (
                  <span className={`list__item__column__item list__item__column__item--${column.replace('.', '_')}`} key={index}>
                    {this.renderValue(column, item)}
                  </span>
								))
							}
              {!nocontext && (
                <ContextMenuTrigger
                  className="list__item__column__item list__item__column__item--context-menu-trigger subtle"
                  onTrigger={this.handleContextMenu}
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
                ? this.renderValue('name')
                : <span className="grey-text">{item.uri}</span>
            }
          </div>

          {details ? (
            <ul className="list__item__column__item list__item__column__item--details details">
              {
							 	details.map((detail, index) => {
							 	  const value = this.renderValue(detail);
							 	  if (!value) return null;
							 	  return (
                    <li
                      className={`details__item details__item--${detail.replace('.', '_')}`}
                      key={index}
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
              middle_column.map((column, index) => (
                <span
                  className={`list__item__column__item list__item__column__item--${column.replace('.', '_')}`}
                  key={index}
                >
                  {this.renderValue(column)}
                </span>
              ))
						}
          </div>
        )}
      </div>
    );
  }
}

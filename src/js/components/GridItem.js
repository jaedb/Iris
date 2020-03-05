
import React from 'react';
import {
  uriType,
  scrollTo,
  sourceIcon,
  uriSource,
} from '../util/helpers';
import Link from './Link';
import Icon from './Icon';
import Thumbnail from './Thumbnail';
import LinksSentence from './LinksSentence';

export default class GridItem extends React.Component {
  componentDidMount() {
    const {
      lastfmActions,
      mopidyActions,
      spotifyActions,
      spotifyAvailable,
      item,
    } = this.props;

    if (!item || item.images) return;

    switch (uriType(item.uri)) {
      case 'artist':
        if (spotifyActions && spotifyAvailable) {
          spotifyActions.getArtistImages(item);
        }
        break;

      case 'album':
        // If Mopidy doesn't find any images, then it will pass on the call to LastFM
        mopidyActions.getImages('albums', [item.uri]);
        break;

      default:
        break;
    }
  }

  shouldComponentUpdate = (nextProps) => {
    const { item } = this.props;
    return nextProps.item !== item;
  }

  onContextMenu = (e) => {
    const { onContextMenu } = this.props;
    if (onContextMenu) {
      onContextMenu(e);
    }
  }

  renderSecondary = (item) => {
    const output = '';
    const link_to = null;

    switch (uriType(item.uri)) {
      case 'playlist':
        if (item.tracks_total) {
          return (
            <span className="grid__item__secondary__content">
              {item.tracks_total}
              {' '}
tracks
            </span>
          );
        }
        break;

      case 'artist':
        return (
          <span className="grid__item__secondary__content">
            {item.followers !== undefined ? `${item.followers.toLocaleString()} followers ` : null}
            {item.albums_uris !== undefined ? `${item.albums_uris.length} albums` : null}
          </span>
        );
        break;

      case 'album':
        return (
          <span className="grid__item__secondary__content">
            {item.artists !== undefined ? <LinksSentence nolinks items={item.artists} /> : null}
          </span>
        );
        break;

      default:
        return (
          <span className="grid__item__secondary__content">
            { item.artists !== undefined ? <LinksSentence nolinks items={item.artists} /> : null }
            { item.followers !== undefined ? `${item.followers.toLocaleString()} followers` : null }
          </span>
        );
    }

    return output;
  }

  render = () => {
    const { item, link: customLink, type, show_source_icon } = this.props;
    if (!item) return null;

    const album = {
      ...item.album,
      added_at: item.album && item.album.added_at,
    };

    let images = null;
    if (album.images) {
      if (Array.isArray(album.images)) {
        images = album.images[0];
      } else {
        images = album.images;
      }
    } else if (item.icons) {
      images = item.icons;
    }

    const link = customLink || `/${type}/${encodeURIComponent(item.uri)}`;

    return (
      <Link
        className={`grid__item grid__item--${type}`}
        to={link}
        onClick={scrollTo}
        onContextMenu={this.onContextMenu}
      >
        <Thumbnail glow size="medium" className="grid__item__thumbnail" images={images} />
        <div className="grid__item__name">
          {item.name ? item.name : <span className="opaque-text">{item.uri}</span>}
        </div>
        <div className="grid__item__secondary">
          {show_source_icon && (
            <Icon name={sourceIcon(item.uri)} type="fontawesome" className="source" />
          )}
          {this.renderSecondary(item)}
        </div>
      </Link>
    );
  }
}

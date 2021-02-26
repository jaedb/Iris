import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Link from './Link';
import Icon from './Icon';
import Thumbnail from './Thumbnail';
import LinksSentence from './LinksSentence';
import { I18n } from '../locale';
import { encodeUri } from '../util/format';
import { scrollTo, sourceIcon, getGridItem } from '../util/helpers';
import * as uiActions from '../services/ui/actions';

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

class GridItem extends React.Component {
  componentDidMount() {
    const {
      mopidyActions,
      spotifyActions,
      spotifyAvailable,
      item,
    } = this.props;

    if (!item || item.images) return;

    switch (item.type) {
      case 'artist':
        if (spotifyActions && spotifyAvailable) {
          //spotifyActions.getArtistImages(item);
        }
        break;

      case 'album':
        // If Mopidy doesn't find any images, then it will pass on the call to LastFM
        if (mopidyActions) {
          mopidyActions.getImages([item.uri]);
        }
        break;

      default:
        break;
    }
  }

  shouldComponentUpdate = (nextProps) => {
    const { item } = this.props;
    return nextProps.item !== item;
  }

  onContextMenu(e, item) {
    const { uiActions: { showContextMenu } } = this.props;

    e.preventDefault();
    showContextMenu({
      e,
      context: item.type,
      uris: [item.uri],
      items: [item],
      tracklist_uri: item.uri,
    });
  }

  render = () => {
    const {
      link: customLink,
      show_source_icon,
      grid_glow_enabled,
      data,
      item: itemProp,
      rowIndex,
      columnIndex,
      columnCount = 5, // TODO: Make this dynamic from the Grid wrapper
      style,
    } = this.props;

    const item = itemProp || getGridItem({ data, columnIndex, rowIndex, columnCount });
    if (!item) return null;
    // Not sure what purpose this serves; squashing our album over top of the item?
    // if (album) item = { ...item, ...album };

    const link = customLink || `/${item.type}/${encodeUri(item.uri)}`;

    return (
      <div style={style}>
        <Link
          className={`grid__item grid__item--${item.type}`}
          to={link}
          onClick={scrollTo}
          onContextMenu={(e) => this.onContextMenu(e, item)}
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
      </div>
    );
  }
}

const mapStateToProps = (state) => {
  const {
    ui: {
      grid_glow_enabled,
    },
  } = state;

  return {
    grid_glow_enabled,
  };
};

export {
  GridItem,
};

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(GridItem);

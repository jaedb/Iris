
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

export default class ListItem extends React.Component {
  constructor(props) {
    super(props);
  }

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

  handleClick(e) {
    // make sure we haven't clicked a nested link (ie Artist name)
    if (e.target.tagName.toLowerCase() !== 'a') {
      e.preventDefault();
      this.props.history.push((this.props.link_prefix ? this.props.link_prefix : '') + encodeURIComponent(this.props.item.uri));
      scrollTo();
    }
  }

  handleMouseDown(e) {
    // make sure we haven't clicked a nested link (ie Artist name)
    if (e.target.tagName.toLowerCase() !== 'a') {
      e.preventDefault();
      this.props.history.push((this.props.link_prefix ? this.props.link_prefix : '') + encodeURIComponent(this.props.item.uri));
      scrollTo();
    }
  }

  handleContextMenu(e) {
    if (this.props.handleContextMenu) {
      e.preventDefault();
      this.props.handleContextMenu(e, this.props.item);
    }
  }

  renderValue(key_string) {
    const key = key_string.split('.');
    let value = Object.assign(this.props.item);

    for (let i = 0; i < key.length; i++) {
      if (value[key[i]] === undefined) {
        return null;
      } if (typeof (value[key[i]]) === 'string' && value[key[i]].replace(' ', '') == '') {
        return null;
      }
      value = value[key[i]];
    }

    if (key_string === 'tracks_total' || key_string === 'tracks_uris.length') {
      return (
        <span>
          <I18n path="specs.tracks" count={value} />
        </span>
      );
    }
    if (key_string === 'followers') {
      return (
        <span>
          <I18n path="specs.followers" count={value.toLocaleString()} />
        </span>
      );
    }
    if (key_string === 'added_at') {
      return (
        <span>
          <I18n path="specs.added_ago" time={dater('ago', value)} />
        </span>
      );
    }
    if (key_string === 'last_modified') {
      return (
        <span>
          <I18n path="specs.updated_ago" time={dater('ago', value)} />
        </span>
      );
    }
    if (key_string === 'owner') return <URILink type="user" uri={value.uri}>{value.id}</URILink>;
    if (key_string === 'popularity') return <Popularity full popularity={value} />;
    if (key_string === 'artists') return <LinksSentence items={value} />;
    if (value === true) return <Icon name="check" />;
    if (typeof (value) === 'number') return <span>{value.toLocaleString()}</span>;
    return <span>{value}</span>;
  }

  render() {
    const { item } = this.props;
    if (!item) {
      return null;
    }

    let class_name = 'list__item';
    if (item.type) {
      class_name += ` list__item--${item.type}`;
    }

    if (this.props.middle_column) {
      class_name += ' list__item--has-middle-column';
    }

    if (this.props.thumbnail) {
      class_name += ' list__item--has-thumbnail';
    }

    if (this.props.details) {
      class_name += ' list__item--has-details';
    }

    return (
      <div
        className={class_name}
        onClick={(e) => this.handleClick(e)}
        onContextMenu={(e) => this.handleContextMenu(e)}
      >

        {this.props.right_column && !this.props.nocontext
						&& (
<div className="list__item__column list__item__column--right">
  {
								(this.props.right_column ? this.props.right_column.map((column, index) => (
  <span className={`list__item__column__item list__item__column__item--${column.replace('.', '_')}`} key={index}>
    {this.renderValue(column, item)}
  </span>
								)) : null)
							}

  {this.props.nocontext ? null : <ContextMenuTrigger className="list__item__column__item list__item__column__item--context-menu-trigger subtle" onTrigger={(e) => this.handleContextMenu(e)} />}

</div>
						)}

        <div className="list__item__column list__item__column--name">

          {this.props.thumbnail ? <Thumbnail className="list__item__column__item list__item__column__item--thumbnail" images={(item.images ? item.images : null)} size="small" /> : null}

          <div className="list__item__column__item list__item__column__item--name">
            {item.name !== undefined ? this.renderValue('name') : <span className="grey-text">{item.uri}</span>}
          </div>

          {this.props.details ? (
            <ul className="list__item__column__item list__item__column__item--details details">
              {
							 	this.props.details.map((detail, index) => {
							 	  const value = this.renderValue(detail);

							 	  if (!value) {
							 	    return null;
							 	  }

							 	  return (
  <li className={`details__item details__item--${detail.replace('.', '_')}`} key={index}>
    {value}
  </li>
							 	  );
							 	})
							}
            </ul>
          ) : null}
        </div>

        {this.props.middle_column ? (
          <div className="list__item__column list__item__column--middle">
            {
							(this.props.middle_column ? this.props.middle_column.map((column, index) => (
  <span className={`list__item__column__item list__item__column__item--${column.replace('.', '_')}`} key={index}>
    {this.renderValue(column)}
  </span>
							)) : null)
						}
          </div>
        ) : null}
      </div>
    );
  }
}

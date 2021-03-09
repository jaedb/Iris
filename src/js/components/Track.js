import React from 'react';
import { useSelector } from 'react-redux';
import Icon from './Icon';
import LinksSentence from './LinksSentence';
import { Dater, dater } from './Dater';
import URILink from './URILink';
import ContextMenuTrigger from './ContextMenuTrigger';
import ErrorBoundary from './ErrorBoundary';
import {
  getFromUri,
  titleCase,
  isTouchDevice,
  sourceIcon,
  uriType,
} from '../util/helpers';
import { I18n, i18n } from '../locale';
import { getDragger } from '../util/selectors';

export default class Track extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      hover: false,
    };

    this.key = props.buildTrackKey(props.item, props.getItemIndex());
    this.start_time = 0;
    this.end_time = 0;
    this.start_position = false;
  }

  handleMouseEnter = () => {
    this.setState({ hover: true });
  }

  handleMouseLeave = () => {
    this.setState({ hover: false });
  }

  handleContextMenu = (e) => {
    const { handleContextMenu } = this.props;
    handleContextMenu(e, this.key);
  }

  handleMouseDown = (e) => {
    const target = $(e.target);

    // Clicked a nested link (ie Artist name), so no dragging required
    if (target.is('a')) {
      return false;
    }

    // Only listen for left mouse clicks
    if (e.button === 0) {
      this.start_position = {
        x: e.pageX,
        y: e.pageY,
      };

      // Not left click, then ensure no dragging
    } else {
      this.start_position = false;
    }
  }

  handleMouseMove = (e) => {
    const {
      handleDrag,
    } = this.props;

    if (handleDrag === undefined) return false;

    if (this.start_position) {
      const start_x = this.start_position.x;
      const start_y = this.start_position.y;
      const threshold = 5;

      // Have we dragged outside of our threshold zone?
      if (
        e.pageX > start_x + threshold
        || e.pageX < start_x - threshold
        || e.pageY > start_y + threshold
        || e.pageY < start_y - threshold
      ) {
        // Handover to parent for dragging. We can unset all our behaviour now.
        handleDrag(e, this.key);
        this.start_position = false;
      }
    }
  }

  handleMouseUp = (e) => {
    const {
      handleDrop,
      handleClick,
      dragger,
    } = this.props;
    const target = $(e.target);

    // Only listen for left clicks
    if (e.button === 0) {
      if (dragger) {
        e.preventDefault();

        if (handleDrop !== undefined) {
          handleDrop(e, this.key);
        }
      } else if (!target.is('a') && target.closest('a').length <= 0) {
        handleClick(e, this.key);
        this.start_position = false;
      }
      return;
    }

    // Not left click, then ensure no dragging
    this.start_position = false;
  }

  handleDoubleClick = (e) => {
    const { handleDoubleClick } = this.props;

    handleDoubleClick(e, this.key);
  }

  handleTouchStart = (e) => {
    const { handleTouchDrag } = this.props;
    const target = $(e.target);
    const timestamp = Math.floor(Date.now());

    if (target.hasClass('drag-zone')) {
      handleTouchDrag(e, this.key);
      e.preventDefault();
    }

    // Save touch start details
    this.start_time = timestamp;
    this.start_position = {
      x: e.touches[0].clientX,
      y: e.touches[0].clientY,
    };

    return false;
  }

  handleTouchEnd = (e) => {
    const {
      handleDoubleTap,
      handleContextMenu,
      handleTap,
    } = this.props;

    const target = $(e.target);
    const timestamp = Math.floor(Date.now());
    const tap_distance_threshold = 10;		// Max distance (px) between touchstart and touchend to qualify as a tap
    const tap_time_threshold = 200;			// Max time (ms) between touchstart and touchend to qualify as a tap
    const end_position = {
      x: e.changedTouches[0].clientX,
      y: e.changedTouches[0].clientY,
    };

    // Too long between touchstart and touchend
    if (this.start_time + tap_time_threshold < timestamp) {
      return false;
    }

    // Make sure there's enough distance between start and end before we handle
    // this event as a 'tap'
    if (
      this.start_position.x + tap_distance_threshold > end_position.x
      && this.start_position.x - tap_distance_threshold < end_position.x
      && this.start_position.y + tap_distance_threshold > end_position.y
      && this.start_position.y - tap_distance_threshold < end_position.y
    ) {
      // Clicked a nested link (ie Artist name), so no dragging required
      if (!target.is('a')) {
        e.preventDefault();
      }

      // Context trigger
      if (target.hasClass('touch-contextable')) {
        // Update our selection. By not passing touch = true selection will work like a regular
        // click this.props.handleSelection(e);
        handleContextMenu(e, this.key);
        return false;
      }

      // We received a touchend within 300ms ago, so handle as double-tap
      if ((timestamp - this.end_time) > 0 && (timestamp - this.end_time) <= 300) {
        handleDoubleTap(e, this.key);
        e.preventDefault();
        return false;
      }

      handleTap(e, this.key);
    }

    this.end_time = timestamp;
  }

  renderTrackMiddleColumn = () => {
    const {
      track_context,
      item: {
        added_from,
        added_by,
        played_at,
      } = {},
    } = this.props;

    let content;

    switch (track_context) {
      case 'history': {
        content = (
          <div className="list__item__column__item list__item__column__item--played_at">
            {
              played_at ? (
                <I18n path="specs.played_ago" time={dater('ago', played_at)} />
              ) : ('-')
            }
          </div>
        );
        break;
      }

      case 'queue': {
        if (added_from && added_by) {
          const type = (added_from ? uriType(added_from) : null);

          switch (type) {
            case 'discover':
              var link = (
                <URILink type="recommendations" uri={getFromUri('seeds', added_from)}>
                  <I18n path="discover.title" />
                </URILink>
              );
              break;

            case 'browse':
              var link = (
                <URILink type={type} uri={added_from}>
                  <I18n path="library.browse.title" />
                </URILink>
              );
              break;

            case 'search':
              var link = (
                <URILink type={type} uri={added_from}>
                  <I18n path="search.title" />
                </URILink>
              );
              break;

            case 'radio':
              var link = <I18n path="modal.edit_radio.title" />;
              break;

            case 'queue-history':
              var link = <I18n path="queue_history.title" />;
              break;

            default:
              var link = <URILink type={type} uri={added_from}>{titleCase(type)}</URILink>;
          }

          content = (
            <div className="list__item__column__item list__item__column__item--added">
              <span className="from">
                {link}
              </span>
              <span className="by by--with-spacing">
                {`${added_by}`}
              </span>
            </div>
          );
        } else if (added_by) {
          content = (
            <div className="list__item__column__item list__item__column__item--added">
              <span className="by">{added_by}</span>
            </div>
          );
        }
        break;
      }

      default:
        return null;
    }

    return (
      <div className="list__item__column list__item__column--middle">
        {content}
      </div>
    );
  }

  render = () => {
    const {
      item,
      track_context,
      stream_title,
      play_state,
      selected_tracks,
      can_sort,
      show_source_icon,
    } = this.props;
    const {
      hover,
    } = this.state;

    if (!item) return null;

    let className = 'list__item list__item--track mouse-draggable mouse-selectable mouse-contextable';
    const track_details = [];

    if (item.artists) {
      track_details.push(
        <li className="details__item details__item--artists" key="artists">
          {item.artists ? <LinksSentence items={item.artists} type="artist" /> : '-'}
        </li>,
      );
    } else if (item.playing && stream_title) {
      track_details.push(
        <li className="details__item details__item--artists" key="stream_title">
          <span className="links-sentence">{stream_title}</span>
        </li>,
      );
    }

    if (item.album) {
      if (item.album.uri) {
        var album = <URILink type="album" uri={item.album.uri}>{item.album.name}</URILink>;
      } else {
        var album = <span>{item.album.name}</span>;
      }

      track_details.push(
        <li className="details__item details__item--album" key="album">
          {album}
        </li>,
      );
    }

    // If we're touchable, and can sort this tracklist
    let drag_zone = null;
    if (isTouchDevice() && can_sort) {
      className += ' list__item--has-drag-zone';

      drag_zone = (
        <span
          className="list__item__column__item list__item__column__item--drag-zone drag-zone touch-draggable mouse-draggable"
          key="drag-zone"
        >
          <Icon name="drag_indicator" />
        </span>
      );
    }

    const track_middle_column = this.renderTrackMiddleColumn();

    if (selected_tracks.includes(this.key)) className += ' list__item--selected';
    if (can_sort) className += ' list__item--can-sort';
    if (item.type !== undefined) className += ` list__item--${item.type}`;
    if (item.playing) className += ' list__item--playing';
    if (item.loading) className += ' list__item--loading';
    if (hover) className += ' list__item--hover';
    if (track_middle_column) className += ' list__item--has-middle-column';
    if (track_details.length > 0) className += ' list__item--has-details';

    return (
      <ErrorBoundary>
        <div
          className={className}
          onMouseEnter={this.handleMouseEnter}
          onMouseLeave={this.handleMouseLeave}
          onMouseDown={this.handleMouseDown}
          onMouseUp={this.handleMouseUp}
          onMouseMove={this.handleMouseMove}
          onDoubleClick={this.handleDoubleClick}
          onContextMenu={this.handleContextMenu}
          onTouchStart={this.handleTouchStart}
          onTouchEnd={this.handleTouchEnd}
        >
          <div className="list__item__column list__item__column--name">
            <div className="list__item__column__item--name">
              {item.name ? item.name : <span className="mid_grey-text">{item.uri}</span>}
              {item.playing && <Icon className={`js--${play_state}`} name="playing" type="css" />}
            </div>
            {track_details && (
              <ul className="list__item__column__item--details">
                {track_details}
              </ul>
            )}
          </div>
          {track_middle_column}
          <div className="list__item__column list__item__column--right">
            {drag_zone}
            {item.is_explicit && <span className="flag flag--dark">EXPLICIT</span>}
            {(track_context === 'album' || track_context === 'artist') && item.track_number && (
              <span className="mid_grey-text list__item__column__item list__item__column__item--track-number">
                <span>
                  <I18n path="track.title" />
                  &nbsp;
                </span>
                {item.track_number}
              </span>
            )}
            <span className="list__item__column__item list__item__column__item--duration">
              {item.duration ? <Dater type="length" data={item.duration} /> : '-'}
            </span>
            {show_source_icon && (
              <span className="list__item__column__item list__item__column__item--source">
                <Icon type="fontawesome" name={sourceIcon(item.uri)} fixedWidth />
              </span>
            )}
            <ContextMenuTrigger className="list__item__column__item--context-menu-trigger subtle" onTrigger={(e) => this.props.handleContextMenu(e)} />
          </div>
        </div>
      </ErrorBoundary>
    );
  }
}

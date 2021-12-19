import React, { useState } from 'react';
import Icon, { SourceIcon } from './Icon';
import LinksSentence from './LinksSentence';
import { Dater, dater } from './Dater';
import URILink from './URILink';
import ContextMenuTrigger from './ContextMenu/ContextMenuTrigger';
import ErrorBoundary from './ErrorBoundary';
import { isTouchDevice } from '../util/helpers';
import { I18n, i18n } from '../locale';
import AddedFrom from './AddedFrom';

const MiddleColumn = ({
  context,
  item: {
    added_from,
    added_by,
    played_at,
  } = {},
}) => {
  let content;
  switch (context?.type) {
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
      content = (
        <AddedFrom
          from={added_from}
          by={added_by}
          className="list__item__column__item list__item__column__item--added"
        />
      );
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

const Track = ({
  item,
  context,
  stream_title,
  play_state,
  selected_tracks,
  is_selected,
  can_sort,
  show_source_icon,
  getItemIndex,
  handleDrag,
  handleDrop,
  handleDoubleTap,
  handleTouchDrag,
  handleTap,
  dragger,
  onContextMenu,
  onClick,
  onDoubleClick,
}) => {
  const [hover, setHover] = useState(false);
  const index = getItemIndex();
  const [currentEvent, setCurrentEvent] = useState({
    start_time: 0,
    end_time: 0,
  });

  // const onMouseEnter = () => setHover(true);
  // const onMouseLeave = () => setHover(false);
  // const onDoubleClick = (e) => handleDoubleClick(e, key);
  // const updateCurrentEvent = (data) => setCurrentEvent((prev) => ({ ...prev, ...data }));

  const handleContextMenu = (e) => onContextMenu(item, index, e);
  const handleClick = (e) => onClick(item, index, e);
  const handleDoubleClick = (e) => onDoubleClick(item, index, e);

  // const onMouseDown = (e) => {
  //   const target = $(e.target);

  //   // Clicked a nested link (ie Artist name), so no dragging required
  //   if (target.is('a')) {
  //     return false;
  //   }

  //   // Only listen for left mouse clicks
  //   if (e.button === 0) {
  //     updateCurrentEvent({
  //       start_position: {
  //         x: e.pageX,
  //         y: e.pageY,
  //       },
  //     });

  //     // Not left click, then ensure no dragging
  //   } else {
  //     updateCurrentEvent({ start_position: false });
  //   }
  // };

  // const onMouseMove = (e) => {
  //   if (handleDrag === undefined) return false;

  //   if (currentEvent.start_position) {
  //     const start_x = currentEvent.start_position.x;
  //     const start_y = currentEvent.start_position.y;
  //     const threshold = 5;

  //     // Have we dragged outside of our threshold zone?
  //     if (
  //       e.pageX > start_x + threshold
  //       || e.pageX < start_x - threshold
  //       || e.pageY > start_y + threshold
  //       || e.pageY < start_y - threshold
  //     ) {
  //       // Handover to parent for dragging. We can unset all our behaviour now.
  //       handleDrag(e, key);
  //       updateCurrentEvent({ start_position: false });
  //     }
  //   }
  // };

  // const onMouseUp = (e) => {
  //   const target = $(e.target);

  //   // Only listen for left clicks
  //   if (e.button === 0) {
  //     if (dragger) {
  //       e.preventDefault();

  //       if (handleDrop !== undefined) {
  //         handleDrop(e, key);
  //       }
  //     } else if (!target.is('a') && target.closest('a').length <= 0) {
  //       handleClick(e, key);
  //       updateCurrentEvent({ start_position: false });
  //     }
  //     return;
  //   }

  //   // Not left click, then ensure no dragging
  //   updateCurrentEvent({ start_position: false });
  // }


  // const onTouchStart = (e) => {
  //   const target = $(e.target);
  //   const timestamp = Math.floor(Date.now());

  //   if (target.hasClass('drag-zone')) {
  //     handleTouchDrag(e, key);
  //     e.preventDefault();
  //   }

  //   // Save touch start details
  //   updateCurrentEvent({
  //     start_time: timestamp,
  //     start_position: {
  //       x: e.touches[0].clientX,
  //       y: e.touches[0].clientY,
  //     },
  //   });
  //   return false;
  // }

  // const onTouchEnd = (e) => {
  //   const target = $(e.target);
  //   const timestamp = Math.floor(Date.now());
  //   const tap_distance_threshold = 10; // Max distance (px) between touchstart and touchend to qualify as a tap
  //   const tap_time_threshold = 200; // Max time (ms) between touchstart and touchend to qualify as a tap
  //   const end_position = {
  //     x: e.changedTouches[0].clientX,
  //     y: e.changedTouches[0].clientY,
  //   };

  //   // Too long between touchstart and touchend
  //   if (currentEvent.start_time + tap_time_threshold < timestamp) {
  //     return false;
  //   }

  //   // Make sure there's enough distance between start and end before we handle
  //   // this event as a 'tap'
  //   if (
  //     currentEvent.start_position.x + tap_distance_threshold > end_position.x
  //     && currentEvent.start_position.x - tap_distance_threshold < end_position.x
  //     && currentEvent.start_position.y + tap_distance_threshold > end_position.y
  //     && currentEvent.start_position.y - tap_distance_threshold < end_position.y
  //   ) {
  //     // Clicked a nested link (ie Artist name), so no dragging required
  //     if (!target.is('a')) {
  //       e.preventDefault();
  //     }

  //     // Context trigger
  //     if (target.hasClass('touch-contextable')) {
  //       // Update our selection. By not passing touch = true selection will work like a regular
  //       // click this.props.handleSelection(e);
  //       handleContextMenu(e, key);
  //       return false;
  //     }

  //     // We received a touchend within 300ms ago, so handle as double-tap
  //     if ((timestamp - currentEvent.end_time) > 0 && (timestamp - currentEvent.end_time) <= 300) {
  //       handleDoubleTap(e, key);
  //       e.preventDefault();
  //       return false;
  //     }

  //     handleTap(e, key);
  //   }

  //   updateCurrentEvent({ end_time: timestamp });
  // }

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
    track_details.push(
      <li className="details__item details__item--album" key="album">
        {item.album.uri
          ? <URILink type="album" uri={item.album.uri}>{item.album.name}</URILink>
          : <span>{item.album.name}</span>
        }
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

  const track_middle_column = <MiddleColumn context={context} item={item} />;
  if (is_selected(getItemIndex())) className += ' list__item--selected';
  if (can_sort) className += ' list__item--can-sort';
  if (item.type !== undefined) className += ` list__item--${item.type}`;
  if (item.playing) className += ' list__item--playing';
  if (item.loading) className += ' list__item--loading';
  if (item.is_playable === false) className += ' list__item--disabled';
  if (hover) className += ' list__item--hover';
  if (track_middle_column) className += ' list__item--has-middle-column';
  if (track_details.length > 0) className += ' list__item--has-details';

  return (
    <ErrorBoundary>
      <div
        className={className}
        onClick={handleClick}
        onDoubleClick={handleDoubleClick}
        onContextMenu={handleContextMenu}
        // onMouseEnter={onMouseEnter}
        // onMouseLeave={onMouseLeave}
        // onMouseDown={onMouseDown}
        // onMouseUp={onMouseUp}
        // onMouseMove={onMouseMove}
        // onDoubleClick={onDoubleClick}
        // onContextMenu={onContextMenu}
        // onTouchStart={onTouchStart}
        // onTouchEnd={onTouchEnd}
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
          {item.is_explicit && <span className="flag flag--dark">{i18n('track.explicit').toUpperCase()}</span>}
          {item.is_playable === false && <span className="flag flag--dark">{i18n('track.unplayable').toUpperCase()}</span>}
          {(context?.type === 'album' || context?.type === 'artist') && item.track_number && (
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
              <SourceIcon uri={item.uri} fixedWidth />
            </span>
          )}
          <ContextMenuTrigger
            className="list__item__column__item--context-menu-trigger subtle"
            onTrigger={handleContextMenu}
          />
        </div>
      </div>
    </ErrorBoundary>
  );
}

export default Track;

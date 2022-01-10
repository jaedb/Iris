import React, { useRef } from 'react';
import { useDrag, useDrop } from 'react-dnd';
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
  switch (context ?.type) {
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
  is_selected,
  can_sort,
  show_source_icon,
  getItemIndex,
  getDragItem,
  events,
}) => {
  const index = getItemIndex();
  const ref = useRef(null);
  const [{ handlerId, isOver }, drop] = useDrop({
    accept: 'TRACK',
    collect: (monitor) => ({
      handlerId: monitor.getHandlerId(),
      isOver: can_sort && monitor.isOver(),
    }),
    canDrop: () => can_sort,
    drop: () => {
      events.onDrop(index);
    },
  });
  const [{ isDragging }, drag] = useDrag({
    type: 'TRACK',
    item: getDragItem(item, index),
    collect: (monitor) => ({
      isDragging: monitor.isDragging(),
    }),
  });
  drag(drop(ref));

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
  if (is_selected(index)) className += ' list__item--selected';
  if (isOver) className += ' list__item--drag-over';
  if (isDragging) className += ' list__item--dragging';
  if (can_sort) className += ' list__item--can-sort';
  if (item.type !== undefined) className += ` list__item--${item.type}`;
  if (item.playing) className += ' list__item--playing';
  if (item.loading) className += ' list__item--loading';
  if (item.is_playable === false) className += ' list__item--disabled';
  if (track_middle_column) className += ' list__item--has-middle-column';
  if (track_details.length > 0) className += ' list__item--has-details';

  const onClick = (e) => events.onClick(item, index, e);
  const onMouseDown = (e) => events.onMouseDown(item, index, e);
  const onDoubleClick = (e) => events.onDoubleClick(item, index, e);
  const onContextMenu = (e) => events.onContextMenu(item, index, e);

  return (
    <ErrorBoundary>
      <div
        className={className}
        onMouseDown={onMouseDown}
        onDoubleClick={onDoubleClick}
        onContextMenu={onContextMenu}
        ref={ref}
        data-handler-id={handlerId}
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
          {(context ?.type === 'album' || context ?.type === 'artist') && item.track_number && (
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
            onTrigger={onContextMenu}
          />
        </div>
      </div>
    </ErrorBoundary>
  );
}

export default Track;

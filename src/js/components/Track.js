
import React from 'react';
import Icon from './Icon';
import LinksSentence from './LinksSentence';
import Dater from './Dater';
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

export default class Track extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      hover: false,
    };

    this.start_time = 0;
    this.end_time = 0;
    this.start_position = false;
  }

  handleMouseEnter(e) {
    this.setState({ hover: true });
  }

  handleMouseLeave(e) {
    this.setState({ hover: false });
  }

  handleMouseDown(e) {
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

  handleMouseMove(e) {
    const target = $(e.target);

    // No drag handling means NO
    if (this.props.handleDrag === undefined) {
      return false;
    }

    if (this.start_position) {
      const start_x = this.start_position.x;
      const start_y = this.start_position.y;
      const threshold = 5;

      // Have we dragged outside of our threshold zone?
      if (e.pageX > start_x + threshold || e.pageX < start_x - threshold || e.pageY > start_y + threshold || e.pageY < start_y - threshold) {
        // Handover to parent for dragging. We can unset all our behaviour now.
        this.props.handleDrag(e);
        this.start_position = false;
      }
    }
  }

  handleMouseUp(e) {
    const target = $(e.target);

    // Only listen for left clicks
    if (e.button === 0) {
      if (this.props.dragger) {
        e.preventDefault();

        if (this.props.handleDrop !== undefined) {
          this.props.handleDrop(e);
        }
      } else if (!target.is('a') && target.closest('a').length <= 0) {
        this.props.handleClick(e);
        this.start_position = false;
      }

      // Not left click, then ensure no dragging
    } else {
      this.start_position = false;
      return false;
    }
  }

  handleDoubleClick(e) {
    this.props.handleDoubleClick(e);
  }

  handleTouchStart(e) {
    const target = $(e.target);
    const timestamp = Math.floor(Date.now());

    // Touch-drag zone
    if (target.hasClass('drag-zone')) {
      this.props.handleTouchDrag(e);
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

  handleTouchEnd(e) {
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
    if (this.start_position.x + tap_distance_threshold > end_position.x
			&& this.start_position.x - tap_distance_threshold < end_position.x
			&& this.start_position.y + tap_distance_threshold > end_position.y
			&& this.start_position.y - tap_distance_threshold < end_position.y) {
      // Clicked a nested link (ie Artist name), so no dragging required
      if (!target.is('a')) {
        e.preventDefault();
      }

      // Context trigger
      if (target.hasClass('touch-contextable')) {
        // Update our selection. By not passing touch = true selection will work like a regular click
        // this.props.handleSelection(e);
        this.props.handleContextMenu(e);
        return false;
      }

      // We received a touchend within 300ms ago, so handle as double-tap
      if ((timestamp - this.end_time) > 0 && (timestamp - this.end_time) <= 300) {
        this.props.handleDoubleTap(e);
        e.preventDefault();
        return false;
      }

      this.props.handleTap(e);
    }

    this.end_time = timestamp;
  }

  render() {
    if (!this.props.track) {
      return null;
    }

    const { track } = this.props;
    let className = 'list__item list__item--track mouse-draggable mouse-selectable mouse-contextable';
    const track_details = [];
    const track_actions = [];

    if (track.artists) {
      track_details.push(
        <li className="details__item details__item--artists" key="artists">
          {track.artists ? <LinksSentence items={track.artists} /> : '-'}
        </li>,
      );
    }

    if (track.album) {
      if (track.album.uri) {
        var album = <URILink type="album" uri={track.album.uri}>{track.album.name}</URILink>;
      } else {
        var album = <span>{track.album.name}</span>;
      }

      track_details.push(
        <li className="details__item details__item--album" key="album">
          {album}
        </li>,
      );
    }

    if (this.props.track_context == 'history') {
      var track_middle_column = (
        <div className="list__item__column__item list__item__column__item--played_at">
          {track.played_at ? (
            <span>
              <Dater type="ago" data={track.played_at} />
              {' ago'}
            </span>
          ) : '-'}
        </div>
      );
    } else if (this.props.track_context == 'queue') {
      if (track.added_from && track.added_by) {
        const type = (track.added_from ? uriType(track.added_from) : null);

        switch (type) {
          case 'discover':
            var link = <URILink type="recommendations" uri={getFromUri('seeds', track.added_from)}>Discover</URILink>;
            break;

          case 'browse':
            var link = <URILink uri={track.added_from}>Browse</URILink>;
            break;

          case 'search':
            var link = <URILink uri={track.added_from}>Search</URILink>;
            break;

          case 'radio':
            var link = <span>Radio</span>;
            break;

          case 'queue-history':
            var link = <span>Queue history</span>;
            break;

          default:
            var link = <URILink type={type} uri={track.added_from}>{titleCase(type)}</URILink>;
        }

        var track_middle_column = (
          <div className="list__item__column__item list__item__column__item--added">
            <span className="from">
              {link}
            </span>
            <span className="by by--with-spacing">
              {`${track.added_by}`}
            </span>
          </div>
        );
      } else if (track.added_by) {
        var track_middle_column = (
          <div className="list__item__column__item list__item__column__item--added">
            <span className="by">{track.added_by}</span>
          </div>
        );
      }
    }

    // If we're touchable, and can sort this tracklist
    let drag_zone = null;
    if (isTouchDevice() && this.props.can_sort) {
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

    if (this.props.selected)		className += ' list__item--selected';
    if (this.props.can_sort)		className += ' list__item--can-sort';
    if (track.type !== undefined)	className += ` list__item--${track.type}`;
    if (track.playing)				className += ' list__item--playing';
    if (this.state.hover)			className += ' list__item--hover';
    if (track_middle_column)		className += ' list__item--has-middle-column';
    if (track_details.length > 0)	className += ' list__item--has-details';

    return (
      <ErrorBoundary>
        <div
          className={className}
          onMouseEnter={(e) => this.handleMouseEnter(e)}
          onMouseLeave={(e) => this.handleMouseLeave(e)}
          onMouseDown={(e) => this.handleMouseDown(e)}
          onMouseUp={(e) => this.handleMouseUp(e)}
          onMouseMove={(e) => this.handleMouseMove(e)}
          onDoubleClick={(e) => this.handleDoubleClick(e)}
          onContextMenu={(e) => this.props.handleContextMenu(e)}
          onTouchStart={(e) => this.handleTouchStart(e)}
          onTouchEnd={(e) => this.handleTouchEnd(e)}
        >
          <div className="list__item__column list__item__column--name">
            <div className="list__item__column__item--name">
              {track.name ? track.name : <span className="mid_grey-text">{track.uri}</span>}
              {track.playing ? <Icon className={`js--${this.props.play_state}`} name="playing" type="css" /> : null}
            </div>
            {track_details ? (
              <ul className="list__item__column__item--details">
                {track_details}
              </ul>
            ) : null}
          </div>
          {track_middle_column ? <div className="list__item__column list__item__column--middle">{track_middle_column}</div> : null}
          <div className="list__item__column list__item__column--right">
            {drag_zone}
            {track.is_explicit ? <span className="flag flag--dark">EXPLICIT</span> : null}
            <span className="list__item__column__item list__item__column__item--duration">
              {track.duration ? <Dater type="length" data={track.duration} /> : '-'}
            </span>
            {this.props.show_source_icon ? (
              <span className="list__item__column__item list__item__column__item--source">
                <Icon type="fontawesome" name={sourceIcon(track.uri)} fixedWidth />
              </span>
            ) : null}
            <ContextMenuTrigger className="list__item__column__item--context-menu-trigger subtle" onTrigger={(e) => this.props.handleContextMenu(e)} />
          </div>
        </div>
      </ErrorBoundary>
    );
  }
}

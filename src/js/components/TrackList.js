
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Track from './Track';
import * as mopidyActions from '../services/mopidy/actions';
import * as uiActions from '../services/ui/actions';
import { isTouchDevice } from '../util/helpers';
import { arrayOf } from '../util/arrays';
import { i18n } from '../locale';

class TrackList extends React.Component {
  constructor(props) {
    super(props);

    this.touch_dragging_tracks_keys = false;

    this.handleKeyDown = this.handleKeyDown.bind(this);
    this.handleTouchMove = this.handleTouchMove.bind(this);
    this.handleTouchEnd = this.handleTouchEnd.bind(this);
  }

  componentDidMount() {
    window.addEventListener('keydown', this.handleKeyDown, false);
    window.addEventListener('touchmove', this.handleTouchMove, false);
    window.addEventListener('touchend', this.handleTouchEnd, false);
  }

  componentWillUnmount() {
    window.removeEventListener('keydown', this.handleKeyDown, false);
    window.removeEventListener('touchmove', this.handleTouchMove, false);
    window.removeEventListener('touchend', this.handleTouchEnd, false);
  }

  handleKeyDown(e) {
    const {
      key,
      altKey,
      ctrlKey,
      metaKey,
      shiftKey,
    } = e;

    // When we're focussed on certian elements, don't fire any shortcuts
    // Typically form inputs
    const ignoreNodes = ['INPUT', 'TEXTAREA'];
    if (ignoreNodes.indexOf(e.target.nodeName) > -1) {
      return;
    }

    // Ignore when there are any key modifiers. This enables us to avoid interfering
    // with browser- and OS-default functions.
    if (altKey || ctrlKey || metaKey || shiftKey) {
      return;
    }

    const tracks_keys = this.digestTracksKeys();

    switch (key.toLowerCase()) {
      case 'enter':
        if (tracks_keys && tracks_keys.length > 0) {
          this.playTracks();
        }
        e.preventDefault();
        break;

      case 'backspace':
      case 'delete':
        if (tracks_keys && tracks_keys.length > 0) {
          this.removeTracks();
        }
        e.preventDefault();
        break;

      case 'a':
        var all_tracks = [];
        for (let i = 0; i < this.props.tracks.length; i++) {
          all_tracks.push(this.buildTrackKey(this.props.tracks[i], i));
        }
        this.props.uiActions.setSelectedTracks(all_tracks);
        e.preventDefault();
        break;
    }
  }

  handleDrag(e, track_key) {
    let selected_tracks = [];

    // Dragging a non-selected track. We need to deselect everything
    // else and select only this track
    if (!this.props.selected_tracks.includes(track_key)) {
      this.props.uiActions.setSelectedTracks([track_key]);
      selected_tracks = this.digestTracksKeys([track_key]);
    } else {
      selected_tracks = this.digestTracksKeys();
    }

    const selected_tracks_indexes = arrayOf('index', selected_tracks);

    this.props.uiActions.dragStart(
      e,
      this.props.track_context,
      this.props.uri,
      selected_tracks,
      selected_tracks_indexes,
    );
  }

  handleDrop = (e, track_key) => {
    const {
      dragger: {
        active,
        victims_indexes,
      } = {},
      reorderTracks: doReorderTracks,
    } = this.props;

    if (active && doReorderTracks) {
      const tracks = this.digestTracksKeys([track_key]);
      return doReorderTracks(victims_indexes, tracks[0].index);
    }
    this.touch_dragging_tracks_keys = false;
  }

  handleTouchDrag(e, track_key) {
    // Drag initiated on a selected track
    if (this.props.selected_tracks.includes(track_key)) {
      // They're all dragging
      this.touch_dragging_tracks_keys = this.props.selected_tracks;

      // Not already selected, so just dragging the one track
    } else {
      this.touch_dragging_tracks_keys = [track_key];
      this.props.uiActions.setSelectedTracks([track_key]);
    }
  }

  handleTouchMove(e) {
    if (this.touch_dragging_tracks_keys) {
      const touch = e.touches[0];
      let over = $(document.elementFromPoint(touch.clientX, touch.clientY));
      if (!over.is('.track')) {
        over = over.closest('.list__item--track');
      }
      $(document).find('.touch-drag-hover').removeClass('touch-drag-hover');
      if (over.length > 0) {
        over.addClass('touch-drag-hover');
      }

	        e.returnValue = false;
	        e.cancelBubble = true;
      e.preventDefault();
      e.stopPropagation();
	        return false;
    }
  }

  handleTouchEnd(e) {
    if (this.touch_dragging_tracks_keys) {
      const touch = e.changedTouches[0];
      let over = $(document.elementFromPoint(touch.clientX, touch.clientY));
      if (!over.is('.list__item--track')) {
        over = over.closest('.list__item--track');
      }
      if (over.length > 0) {
        const siblings = over.parent().children('.list__item--track');
        const dropped_at = siblings.index(over);

        if (this.props.reorderTracks !== undefined) {
          this.props.reorderTracks(arrayOf('index', this.digestTracksKeys()), dropped_at);
          this.props.uiActions.setSelectedTracks([]);
        }
      }

      $(document).find('.touch-drag-hover').removeClass('touch-drag-hover');
      $('body').removeClass('touch-dragging');
    }

    this.touch_dragging_tracks_keys = false;
  }

  handleTap(e, track_key) {
    this.updateSelection(e, track_key, true);
  }

  handleDoubleTap(e, track_key) {
    this.playTracks([track_key]);
    this.updateSelection(e, track_key);
  }

  handleClick(e, track_key) {
    this.updateSelection(e, track_key);
  }

  handleDoubleClick(e, track_key) {
    if (this.props.context_menu) {
      this.props.uiActions.hideContextMenu();
    }
    this.playTracks([track_key]);
    this.updateSelection(e, track_key);
  }

  handleContextMenu(e, track_key = null) {
    // Do our best to stop any flow-on events
    e.preventDefault();
    e.stopPropagation();
    e.cancelBubble = true;

    let { selected_tracks } = this.props;

    // Not already selected, so select it prior to triggering menu
    if (track_key && !selected_tracks.includes(track_key)) {
      selected_tracks = [track_key];
      this.props.uiActions.setSelectedTracks(selected_tracks);
    }

    const selected_tracks_digested = this.digestTracksKeys(selected_tracks);
    const selected_tracks_uris = arrayOf('uri', selected_tracks_digested);
    const selected_tracks_indexes = arrayOf('index', selected_tracks_digested);

    const data = {
      e,
      context: (this.props.track_context ? `${this.props.track_context}-track` : 'track'),
      tracklist_uri: (this.props.uri ? this.props.uri : null),
      items: selected_tracks_digested,
      uris: selected_tracks_uris,
      indexes: selected_tracks_indexes,
    };

    this.props.uiActions.showContextMenu(data);
  }

  updateSelection(e, track_key, touched = false) {
    let { selected_tracks } = this.props;

    if ((e.ctrlKey || e.metaKey) || touched) {
      // Already selected, so unselect it
      if (selected_tracks.includes(track_key)) {
        const index = selected_tracks.indexOf(track_key);
        selected_tracks.splice(index, 1);

        // Not selected, so add it
      } else {
        selected_tracks.push(track_key);
      }
    } else if (e.shiftKey) {
      const last_selected_track = this.digestTracksKeys(selected_tracks[selected_tracks.length - 1]);
      const last_selected_track_index = last_selected_track.index;
      const newly_selected_track = this.digestTracksKeys(track_key);
      const newly_selected_track_index = newly_selected_track.index;

      // We've selected a track further down the list,
      // so proceed normally
      if (last_selected_track_index < newly_selected_track_index) {
        var start = last_selected_track_index + 1;
        var end = newly_selected_track_index;

        // Selected a track up the list, so
        // our last selected is the END of our range
      } else {
        var start = newly_selected_track_index;
        var end = last_selected_track_index - 1;
      }

      if (start !== false && start >= 0 && end !== false && end >= 0) {
        for (let i = start; i <= end; i++) {
          selected_tracks.push(this.buildTrackKey(this.props.tracks[i], i));
        }
      }

      // Regular, unmodified left click
    } else {
      selected_tracks = [track_key];
    }

    this.props.uiActions.setSelectedTracks(selected_tracks);
  }

  isRightClick(e) {
    if ('which' in e) {
      return e.which == 3;
    } if ('button' in e) {
      return e.button == 2;
    }
    return false;
  }

  playTracks = (tracks_keys = null) => {
    const {
      uri,
      uiActions: {
        createNotification,
      },
      mopidyActions: {
        playURIs,
      },
      playTracks: doPlayTracks,
    } = this.props;

    const selected_tracks = tracks_keys !== null
      ? this.digestTracksKeys(tracks_keys)
      : this.digestTracksKeys();

    if (selected_tracks.length <= 0) {
      createNotification({ content: i18n('errors.nothing_selected'), level: 'error' });
      return;
    }

    if (doPlayTracks) {
      doPlayTracks(selected_tracks);
      return;
    }
    const selected_tracks_uris = arrayOf('uri', selected_tracks);
    playURIs(selected_tracks_uris, uri);
  }

  removeTracks = () => {
    const {
      uiActions: {
        createNotification,
      },
      removeTracks: doRemoveTracks,
    } = this.props;

    const selected_tracks = this.digestTracksKeys();

    if (!doRemoveTracks) {
      createNotification({ content: `Cannot delete ${selected_tracks.length > 1 ? 'these tracks' : 'this track'}`, level: 'error' });
      return;
    }
    const selected_tracks_indexes = arrayOf('index', selected_tracks);
    doRemoveTracks(selected_tracks_indexes);
  }

  /**
	 * Build the track key
	 * This is our unique reference to a track in a particular tracklist
	 *
	 * @param track = Track obj
	 * @param index = int (position of track in tracklist)
	 * @return string
	 * */
  buildTrackKey(track, index) {
    let key = index;
    key += `@@${track.tlid ? track.tlid : 'none'}`;
    key += `@@${track.uri}`;
    key += `@@${this.props.uri ? this.props.uri : 'none'}`;
    key += `@@${this.props.track_context ? this.props.track_context : 'none'}`;
    return key;
  }

  /**
	 * Digest our selected tracks
	 *
	 * @param tracks = mixed (defaults to stored value)
	 * @param indexex_only = boolean (do we just want an array of indexes)
	 * @return mixed
	 * */
  digestTracksKeys(keys = this.props.selected_tracks, indexes_only = false) {
    if (!keys) {
      return false;
    }

    // Accommodate a single key
    let singleton = false;
    if (!(keys instanceof Array)) {
      singleton = true;
      keys = [keys];
    }

    // Construct a basic track object, based on our unique track key
    // This is enough to perform interactions (dragging, selecting, etc)
    const array = [];
    for (const key of keys) {
      const key_components = key.split('@@');

      if (indexes_only) {
        array.push(key_components[0]);
      } else {
        array.push({
          key,
          index: parseInt(key_components[0]),
          tlid: parseInt(key_components[1]),
          uri: key_components[2],
          context: key_components[3],
          context_uri: key_components[4],
        });
      }
    }

    if (singleton && array.length > 0) {
      return array[0];
    }
    return array;
  }

  render = () => {
    const {
      tracks,
      track_context,
      className: classNameProp = '',
      show_source_icon,
      selected_tracks = [],
      play_state,
      dragger,
      slim_mode,
    } = this.props;

    if (!tracks || Object.prototype.toString.call(tracks) !== '[object Array]') {
      return null;
    }

    return (
      <div className={`list list--tracks ${track_context} ${classNameProp}`}>
        {
					tracks.map(
					  (track, index) => {
					    const key = this.buildTrackKey(track, index);
					    return (
                <Track
                  show_source_icon={show_source_icon}
                  key={key}
                  mini_zones={slim_mode || isTouchDevice()}
                  track={{ ...track, key }}
                  track_context={track_context}
                  can_sort={track_context === 'queue' || track_context === 'editable-playlist'}
                  selected={selected_tracks.includes(key)}
                  play_state={play_state}
                  dragger={dragger}
                  handleClick={(e) => this.handleClick(e, key)}
                  handleDoubleClick={(e) => this.handleDoubleClick(e, key)}
                  handleContextMenu={(e) => this.handleContextMenu(e, key)}
                  handleDrag={(e) => this.handleDrag(e, key)}
                  handleDrop={(e) => this.handleDrop(e, key)}
                  handleTap={(e) => this.handleTap(e, key)}
                  handleDoubleTap={(e) => this.handleDoubleTap(e, key)}
                  handleTouchDrag={(e) => this.handleTouchDrag(e, key)}
                />
					    );
					  },
					)
				}
      </div>
    );
  }
}

const mapStateToProps = (state) => ({
  play_state: state.mopidy.play_state,
  slim_mode: state.ui.slim_mode,
  selected_tracks: state.ui.selected_tracks,
  dragger: state.ui.dragger,
  current_track: state.core.current_track,
  context_menu: state.ui.context_menu,
});

const mapDispatchToProps = (dispatch) => ({
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(TrackList);

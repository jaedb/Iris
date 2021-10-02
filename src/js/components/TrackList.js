import React, { useState, useEffect } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Track from './Track';
import * as mopidyActions from '../services/mopidy/actions';
import * as uiActions from '../services/ui/actions';
import { isTouchDevice } from '../util/helpers';
import { arrayOf } from '../util/arrays';
import { i18n } from '../locale';
import { SmartList } from './SmartList';

const TrackList = ({
  uri,
  track_context,
  className = '',
  show_source_icon,
  play_state,
  slim_mode,
  tracks,
  selected_tracks,
  removeTracks,
  playTracks,
  reorderTracks,
  context_menu,
  dragger,
  uiActions: {
    createNotification,
    setSelectedTracks,
    showContextMenu,
    dragStart,
  },
  mopidyActions: {
    playURIs,
  },
}) => {
  const [touchDraggingKeys, setTouchDraggingKeys] = useState(false);
  useEffect(() => {
    window.addEventListener('keydown', onKeyDown, false);
    window.addEventListener('touchmove', onTouchMove, false);
    window.addEventListener('touchend', onTouchEnd, false);

    return () => {
      window.removeEventListener('keydown', onKeyDown, false);
      window.removeEventListener('touchmove', onTouchMove, false);
      window.removeEventListener('touchend', onTouchEnd, false);
    }
  }, [selected_tracks]);

  const onKeyDown = (e) => {
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

    const tracks_keys = digestTracksKeys();

    switch (key.toLowerCase()) {
      case 'enter':
        if (tracks_keys && tracks_keys.length > 0) {
          onPlayTracks();
        }
        e.preventDefault();
        break;

      case 'backspace':
      case 'delete':
        if (tracks_keys && tracks_keys.length > 0) {
          onRemoveTracks();
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

  const onDrag = (e, track_key) => {
    let nextSelectedTracks = [];

    // Dragging a non-selected track. We need to deselect everything
    // else and select only this track
    if (!selected_tracks.includes(track_key)) {
      setSelectedTracks([track_key]);
      nextSelectedTracks = digestTracksKeys([track_key]);
    } else {
      nextSelectedTracks = digestTracksKeys();
    }

    const nextSelectedTracks_indexes = arrayOf('index', nextSelectedTracks);

    dragStart(
      e,
      track_context,
      uri,
      nextSelectedTracks,
      nextSelectedTracks_indexes,
    );
  }

  const onDrop = (e, track_key) => {
    if (dragger?.active && reorderTracks) {
      const tracks = digestTracksKeys([track_key]);
      return reorderTracks(dragger?.victims_indexes, tracks[0].index);
    }
    setTouchDraggingKeys(false);
  }

  const onTouchDrag = (e, track_key) => {
    // Drag initiated on a selected track
    if (selected_tracks.includes(track_key)) {
      // They're all dragging
      setTouchDraggingKeys(selected_tracks);

      // Not already selected, so just dragging the one track
    } else {
      setTouchDraggingKeys([track_key]);
      setSelectedTracks([track_key]);
    }
  }

  const onTouchMove = (e) => {
    if (touchDraggingKeys) {
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

  const onTouchEnd = (e) => {
    if (touchDraggingKeys) {
      const touch = e.changedTouches[0];
      let over = $(document.elementFromPoint(touch.clientX, touch.clientY));
      if (!over.is('.list__item--track')) {
        over = over.closest('.list__item--track');
      }
      if (over.length > 0) {
        const siblings = over.parent().children('.list__item--track');
        const dropped_at = siblings.index(over);

        if (reorderTracks !== undefined) {
          reorderTracks(arrayOf('index', digestTracksKeys()), dropped_at);
          setSelectedTracks([]);
        }
      }

      $(document).find('.touch-drag-hover').removeClass('touch-drag-hover');
      $('body').removeClass('touch-dragging');
      setTouchDraggingKeys(false);
    }
  }

  const onTap = (e, track_key) => {
    updateSelection(e, track_key, true);
  }

  const onDoubleTap = (e, track_key) => {
    onPlayTracks([track_key]);
    updateSelection(e, track_key);
  }

  const onClick = (e, track_key) => {
    updateSelection(e, track_key);
  }

  const onDoubleClick = (e, track_key) => {
    if (context_menu) hideContextMenu();
    onPlayTracks([track_key]);
    updateSelection(e, track_key);
  }

  const onContextMenu = (e, track_key = null) => {
    // Do our best to stop any flow-on events
    e.preventDefault();
    e.stopPropagation();
    e.cancelBubble = true;

    let nextSelectedTracks = [...selected_tracks];

    // Not already selected, so select it prior to triggering menu
    if (track_key && !nextSelectedTracks.includes(track_key)) {
      nextSelectedTracks = [track_key];
      setSelectedTracks(nextSelectedTracks);
    }

    const selected_tracks_digested = digestTracksKeys(nextSelectedTracks);
    const selected_tracks_uris = arrayOf('uri', selected_tracks_digested);
    const selected_tracks_indexes = arrayOf('index', selected_tracks_digested);

    showContextMenu({
      e,
      context: (track_context ? `${track_context}-track` : 'track'),
      tracklist_uri: uri,
      items: selected_tracks_digested,
      uris: selected_tracks_uris,
      indexes: selected_tracks_indexes,
    });
  }

  const updateSelection = (e, track_key, touched = false) => {
    let nextSelectedTracks = [...selected_tracks];

    if ((e.ctrlKey || e.metaKey) || touched) {
      // Already selected, so unselect it
      if (nextSelectedTracks.includes(track_key)) {
        const index = nextSelectedTracks.indexOf(track_key);
        nextSelectedTracks.splice(index, 1);

        // Not selected, so add it
      } else {
        nextSelectedTracks.push(track_key);
      }
    } else if (e.shiftKey) {
      const last_selected_track = digestTracksKeys(nextSelectedTracks[nextSelectedTracks.length - 1]);
      const last_selected_track_index = last_selected_track.index;
      const newly_selected_track = digestTracksKeys(track_key);
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
          nextSelectedTracks.push(buildTrackKey(tracks[i], i));
        }
      }

      // Regular, unmodified left click
    } else {
      nextSelectedTracks = [track_key];
    }

    setSelectedTracks(nextSelectedTracks);
  }

  const onPlayTracks = (tracks_keys = null) => {
    const selectedKeys = tracks_keys !== null
      ? digestTracksKeys(tracks_keys)
      : digestTracksKeys();

    if (selectedKeys.length <= 0) {
      createNotification({ content: i18n('errors.nothing_selected'), level: 'error' });
      return;
    }

    if (playTracks) {
      playTracks(selectedKeys);
      return;
    }
    playURIs(arrayOf('uri', selectedKeys), uri);
  }

  const onRemoveTracks = () => {
    const selectedKeys = digestTracksKeys();

    if (!removeTracks) {
      createNotification({ content: `Cannot delete ${selectedKeys.length > 1 ? 'these tracks' : 'this track'}`, level: 'error' });
      return;
    }
    const selected_tracks_indexes = arrayOf('index', selectedKeys);
    removeTracks(selected_tracks_indexes);
  }

  /**
	 * Build the track key
	 * This is our unique reference to a track in a particular tracklist
	 *
	 * @param track = Track obj
	 * @param index = int (position of track in tracklist)
	 * @return string
	 * */
  const buildTrackKey = (track, index) => {
    let key = index;
    key += `@@${track.tlid || 'none'}`;
    key += `@@${track.uri}`;
    key += `@@${uri || 'none'}`;
    key += `@@${track_context || 'none'}`;
    return key;
  }

  /**
	 * Digest our selected tracks
	 *
	 * @param tracks = mixed (defaults to stored value)
	 * @param indexex_only = boolean (do we just want an array of indexes)
	 * @return mixed
	 * */
  const digestTracksKeys = (keys = selected_tracks, indexes_only = false) => {
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

  if (!tracks || Object.prototype.toString.call(tracks) !== '[object Array]') {
    return null;
  }

  return (
    <SmartList
      className={`list list--tracks ${track_context} ${className}`}
      items={tracks}
      itemComponent={Track}
      itemProps={{
        dragger,
        buildTrackKey,
        play_state,
        show_source_icon,
        track_context,
        selected_tracks,
        can_sort: track_context === 'queue' || track_context === 'editable-playlist',
        mini_zones: slim_mode || isTouchDevice(),
        handleClick: onClick,
        handleDoubleClick: onDoubleClick,
        handleContextMenu: onContextMenu,
        handleDrag: onDrag,
        handleDrop: onDrop,
        handleTap: onTap,
        handleDoubleTap: onDoubleTap,
        handleTouchDrag: onTouchDrag,
      }}
    />
  );
}

const mapStateToProps = (state) => ({
  play_state: state.mopidy.play_state,
  slim_mode: state.ui.slim_mode,
  selected_tracks: state.ui.selected_tracks,
  dragger: state.ui.dragger,
  current_track: state.core.current_track,
  context_menu: state.ui.context_menu,
  stream_title: state.core.stream_title,
});

const mapDispatchToProps = (dispatch) => ({
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(TrackList);

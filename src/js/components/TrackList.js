import React, { useState, useEffect } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Track from './Track';
import * as mopidyActions from '../services/mopidy/actions';
import * as uiActions from '../services/ui/actions';
import { isTouchDevice, uriSource } from '../util/helpers';
import { arrayOf, indexToArray } from '../util/arrays';
import { i18n } from '../locale';
import { SmartList } from './SmartList';
import { formatSimpleObject } from '../util/format';

const TrackList = ({
  uri,
  context,
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
  const [selected, setSelected] = useState([]);

  useEffect(() => {
    window.addEventListener('keydown', onKeyDown, false);
    // window.addEventListener('touchmove', onTouchMove, false);
    // window.addEventListener('touchend', onTouchEnd, false);

    return () => {
      window.removeEventListener('keydown', onKeyDown, false);
      // window.removeEventListener('touchmove', onTouchMove, false);
      // window.removeEventListener('touchend', onTouchEnd, false);
    }
  }, [selected_tracks]);

  const selectionIndexByItemIndex = (index) => (
    selected && selected.length > 0
      ? selected.findIndex(({ index: selectedIndex }) => selectedIndex === index)
      : -1
  );

  const nextSelected = (prev, item, index, e, sticky = false) => {
    const alreadySelected = selectionIndexByItemIndex(index);

    if (e.shiftKey) {
      const { index: lastSelectedIndex } = selected[selected.length-1] || { index };
      const next = [...prev, { index, item }];
      const from = lastSelectedIndex < index ? lastSelectedIndex : index;
      const to = lastSelectedIndex > index ? lastSelectedIndex : index;
      for (let i = from; i < to; i++) {
        next.push({ index: i, item: tracks[i] });
      }
      return next;
    }
    if (e.ctrlKey) {
      if (alreadySelected >= 0 && !sticky) {
        const next = [...prev];
        next.splice(alreadySelected, 1);
        return next;
      }
      return [...prev, { item, index }];
    }
    return [{ item, index }];
  }

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

    switch (key.toLowerCase()) {
      case 'enter':
        if (selected.length > 0) {
          onPlayTracks();
        }
        e.preventDefault();
        break;
      case 'backspace':
      case 'delete':
        if (selected.length > 0) {
          onRemoveTracks();
        }
        e.preventDefault();
        break;
      case 'a':
        setSelected(tracks.map((item, index) => ({ item, index })));
        e.preventDefault();
        break;
      default:
        break;
    }
  }

  const onClick = (item, index, e) => {
    e.preventDefault();
    e.persist();

    setSelected((prev) => nextSelected(prev, item, index, e));
  };

  const onDoubleClick = (item, index) => {
    // if (context_menu) hideContextMenu();
    setSelected([{ item, index }]);
    playURIs([item.uri], context);
  };

  const onContextMenu = (item, index, e) => {
    // Do our best to stop any flow-on events
    e.preventDefault();
    e.stopPropagation();
    e.cancelBubble = true;

    const alreadySelected = selectionIndexByItemIndex(index);
    let items = [];
    if (alreadySelected > -1) {
      items = selected;
    } else {
      items = nextSelected(selected, item, index, e);
      setSelected(items);
    }
    items = items.map(({ item: selectedItem }) => selectedItem);

    showContextMenu({
      e,
      context,
      ...(items.length === 1
        ? { type: 'track', item: items[0] }
        : { type: 'tracks', items }
      ),
    });
  };

  const onRemoveTracks = () => {
    if (!removeTracks) {
      createNotification({
        content: `Cannot delete ${selected.length > 1 ? 'these tracks' : 'this track'}`,
        level: 'error',
      });
      return;
    }
    removeTracks(selected.map(({ index }) => index));
  };


  /**
	 * Digest our selected tracks
	 *
	 * @param tracks = mixed (defaults to stored value)
	 * @param indexex_only = boolean (do we just want an array of indexes)
	 * @return mixed
	 * */
  // const digestTracksKeys = (keys = selected_tracks, indexes_only = false) => {
  //   if (!keys) {
  //     return false;
  //   }

  //   // Accommodate a single key
  //   let singleton = false;
  //   if (!(keys instanceof Array)) {
  //     singleton = true;
  //     keys = [keys];
  //   }

  //   // Construct a basic track object, based on our unique track key
  //   // This is enough to perform interactions (dragging, selecting, etc)
  //   const array = [];
  //   for (const key of keys) {
  //     const key_components = key.split('@@');

  //     if (indexes_only) {
  //       array.push(key_components[0]);
  //     } else {
  //       array.push({
  //         key,
  //         index: parseInt(key_components[0]),
  //         tlid: parseInt(key_components[1]),
  //         uri: key_components[2],
  //         provider: uriSource(key_components[2]),
  //         context: key_components[3],
  //         context_uri: key_components[4],
  //       });
  //     }
  //   }

  //   if (singleton && array.length > 0) {
  //     return array[0];
  //   }
  //   return array;
  // }

  if (!tracks || Object.prototype.toString.call(tracks) !== '[object Array]') {
    return null;
  }

  const is_selected = (index) => {
    if (selected.length <= 0) return false;
    return selected.filter(({ index: selIndex }) => index === selIndex).length > 0;
  };

  return (
    <SmartList
      className={`list list--tracks ${className}`}
      items={tracks}
      itemComponent={Track}
      itemProps={{
        dragger,
        play_state,
        show_source_icon,
        context,
        selected_tracks,
        can_sort: context?.can_edit,
        is_selected,
        mini_zones: slim_mode || isTouchDevice(),
        onClick,
        onContextMenu,
        onDoubleClick,
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

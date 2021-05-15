import ReactGA from 'react-ga';
import { useDispatch, useSelector } from 'react-redux';
import { useHotkeys } from 'react-hotkeys-hook';
import { indexToArray, sortItems } from '../util/arrays';
import { collate } from '../util/format';

import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as snapcastActions from '../services/snapcast/actions';

const Hotkeys = () => {
  const dispatch = useDispatch();
  const volume = useSelector((state) => state.mopidy.volume);
  const mute = useSelector((state) => state.mopidy.mute);
  const play_state = useSelector((state) => state.mopidy.play_state);
  const play_time_position = useSelector((state) => parseInt(state.mopidy.time_position, 10));
  const dragging = useSelector((state) => state.ui.dragger?.dragging);
  const allow_reporting = useSelector((state) => state.ui.allow_reporting);
  const snapcast_groups = useSelector((state) => state.snapcast.groups);
  const snapcast_clients = useSelector((state) => state.snapcast.clients);
  const show_disconnected_clients = useSelector((state) => state.ui?.snapcast_show_disconnected_clients);
  const prepare = ({ e, label, callback }) => {
    const {
      target,
      altKey,
      ctrlKey,
      metaKey,
      shiftKey,
      key,
    } = e;
    // Ignore buttons (spacebar triggers it); input fields are ignored by the plugin
    if (target.nodeName === 'BUTTON' && key === ' ') return;

    // Ignore when there are any key modifiers. This enables us to avoid interfering
    // with browser- and OS-default functions.
    if (altKey || ctrlKey || metaKey || shiftKey) {
      return;
    }

    if (allow_reporting) {
      ReactGA.event({ category: 'Hotkey', action: e.key, label });
    }
    e.preventDefault();
    callback();
  };

  const getSnapcastGroup = (index) => {
    const simpleGroups = indexToArray(snapcast_groups);
    if (simpleGroups.length <= 0) return false;

    const group = sortItems(simpleGroups, 'name')[index];
    if (!group) return false;

    return collate(group, { clients: snapcast_clients });
  }

  const setSnapcastVolume = (index, adjustment) => {
    const group = getSnapcastGroup(index);
    let { clients: groupClients } = group || {};
    if (!groupClients) return false;
    if (!show_disconnected_clients) {
      groupClients = groupClients.filter((c) => c.connected);
    }

    const groupVolume = groupClients.reduce(
      (acc, client) => acc + (client.volume || 0),
      0,
    ) / groupClients.length;

    if (group.mute) snapcastActions.setGroupMute(group.id, false);
    dispatch(snapcastActions.setGroupVolume(group.id, groupVolume + adjustment, groupVolume));
    dispatch(uiActions.createNotification({
      content: adjustment > 0 ? 'volume_up' : 'volume_down',
      title: group.name,
      type: 'shortcut',
    }));
  };

  const toggleSnapcastMute = (index) => {
    const group = getSnapcastGroup(index);
    const nextMute = group.mute !== true;

    dispatch(snapcastActions.setGroupMute(group.id, nextMute));
    dispatch(uiActions.createNotification({
      content: nextMute ? 'volume_off' : 'volume_up',
      title: group.name,
      type: 'shortcut',
    }));
  };

  useHotkeys('i', (e) => {
    prepare({
      e,
      label: 'Hotkey info',
      callback: () => {
        history.push('/hotkeys');
      },
    });
  });

  useHotkeys('space,p', (e) => prepare({
    e,
    label: 'Play/pause',
    callback: () => {
      if (play_state === 'playing') {
        dispatch(mopidyActions.pause());
        dispatch(uiActions.createNotification({ content: 'pause', type: 'shortcut' }));
      } else {
        dispatch(mopidyActions.play());
        dispatch(uiActions.createNotification({ content: 'play_arrow', type: 'shortcut' }));
      }
    },
  }), {}, [play_state]);

  useHotkeys('s', (e) => prepare({
    e,
    label: 'Stop',
    callback: () => {
      dispatch(mopidyActions.stop());
      dispatch(uiActions.createNotification({ content: 'stop', type: 'shortcut' }));
    },
  }));

  useHotkeys('r', (e) => prepare({
    e,
    label: 'Rewind',
    callback: () => {
      let new_position = play_time_position - 30000;
      if (new_position < 0) {
        new_position = 0;
      }
      dispatch(mopidyActions.setTimePosition(new_position));
      dispatch(uiActions.createNotification({ content: 'fast_rewind', type: 'shortcut' }));
    },
  }));

  useHotkeys('f', (e) => prepare({
    e,
    label: 'Fastforward',
    callback: () => {
      dispatch(mopidyActions.setTimePosition(play_time_position + 30000));
      dispatch(uiActions.createNotification({ content: 'fast_forward', type: 'shortcut' }));
    },
  }));

  useHotkeys(',', (e) => prepare({
    e,
    label: 'Previous',
    callback: () => {
      dispatch(mopidyActions.previous());
      dispatch(uiActions.createNotification({ content: 'skip_previous', type: 'shortcut' }));
    },
  }));

  useHotkeys('.', (e) => prepare({
    e,
    label: 'Next',
    callback: () => {
      dispatch(mopidyActions.next());
      dispatch(uiActions.createNotification({ content: 'skip_next', type: 'shortcut' }));
    },
  }));

  useHotkeys('=,1+=,2+=,3+=,4+=,5+=', (e, handler) => prepare({
    e,
    label: 'Volume up',
    callback: () => {
      if (handler.key === '=') {
        if (volume !== 'false') {
          dispatch(uiActions.createNotification({ content: 'volume_up', title: 'Master', type: 'shortcut' }));
          let nextVolume = volume + 5;
          if (nextVolume > 100) nextVolume = 100;
          dispatch(mopidyActions.setVolume(nextVolume));
          if (mute) {
            dispatch(mopidyActions.setMute(false));
          }
        }
      } else {
        const index = parseInt(handler.key.replace('+='), 10);
        setSnapcastVolume(index - 1, 5);
      }
    },
  }), {}, [volume, mute, snapcast_groups]);

  useHotkeys('-,1+-,2+-,3+-,4+-,5+-', (e, handler) => prepare({
    e,
    label: 'Volume down',
    callback: () => {
      if (handler.key === '-') {
        if (volume !== 'false') {
          dispatch(uiActions.createNotification({ content: 'volume_down', title: 'Master', type: 'shortcut' }));
          let nextVolume = volume - 5;
          if (nextVolume < 0) nextVolume = 0;
          dispatch(mopidyActions.setVolume(nextVolume));
          if (mute) {
            dispatch(mopidyActions.setMute(false));
          }
        }
      } else {
        const index = parseInt(handler.key.replace('+-'), 10);
        setSnapcastVolume(index - 1, -5);
      }
    },
  }), {}, [volume, mute, snapcast_groups]);

  useHotkeys('0,1+0,2+0,3+0,4+0,5+0', (e, handler) => prepare({
    e,
    label: 'Mute/unmute',
    callback: () => {
      if (handler.key === '0') {
        if (mute) {
          dispatch(mopidyActions.setMute(false));
          dispatch(uiActions.createNotification({ content: 'volume_up', title: 'Master', type: 'shortcut' }));
        } else {
          dispatch(mopidyActions.setMute(true));
          dispatch(uiActions.createNotification({ content: 'volume_off', title: 'Master', type: 'shortcut' }));
        }
      } else {
        const index = parseInt(handler.key.replace('+0'), 10);
        toggleSnapcastMute(index - 1);
      }
    },
  }), {}, [volume, mute, snapcast_groups]);

  useHotkeys('escape', (e) => prepare({
    e,
    label: 'Escape',
    callback: () => {
      if (dragging) {
        dispatch(uiActions.dragEnd());
        e.preventDefault();
      } else if ($('body').hasClass('modal-open')) {
        window.history.back();
        e.preventDefault();
      }
    },
  }), {}, [dragging]);

  return null;
};

export default Hotkeys;

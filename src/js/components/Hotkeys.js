import ReactGA from 'react-ga';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { useHotkeys } from 'react-hotkeys-hook';
import { indexToArray, sortItems } from '../util/arrays';
import { collate } from '../util/format';

import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as snapcastActions from '../services/snapcast/actions';

const Hotkeys = ({
  play_state,
  mopidyActions,
  uiActions,
  snapcastActions,
  mute,
  play_time_position,
  history,
  dragging,
  allow_reporting,
  volume,
  snapcast_groups,
  snapcast_clients,
  show_disconnected_clients,
}) => {
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
    snapcastActions.setGroupVolume(group.id, groupVolume + adjustment, groupVolume);
    uiActions.createNotification({
      content: adjustment > 0 ? 'volume_up' : 'volume_down',
      title: group.name,
      type: 'shortcut',
    });
  };

  const toggleSnapcastMute = (index) => {
    const group = getSnapcastGroup(index);
    const nextMute = group.mute !== true;

    snapcastActions.setGroupMute(group.id, nextMute);
    uiActions.createNotification({
      content: nextMute ? 'volume_off' : 'volume_up',
      title: group.name,
      type: 'shortcut',
    });
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
        mopidyActions.pause();
        uiActions.createNotification({ content: 'pause', type: 'shortcut' });
      } else {
        mopidyActions.play();
        uiActions.createNotification({ content: 'play_arrow', type: 'shortcut' });
      }
    },
  }), {}, [play_state]);

  useHotkeys('s', (e) => prepare({
    e,
    label: 'Stop',
    callback: () => {
      mopidyActions.stop();
      uiActions.createNotification({ content: 'stop', type: 'shortcut' });
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
      mopidyActions.setTimePosition(new_position);
      uiActions.createNotification({ content: 'fast_rewind', type: 'shortcut' });
    },
  }));

  useHotkeys('f', (e) => prepare({
    e,
    label: 'Fastforward',
    callback: () => {
      mopidyActions.setTimePosition(play_time_position + 30000);
      uiActions.createNotification({ content: 'fast_forward', type: 'shortcut' });
    },
  }));

  useHotkeys(',', (e) => prepare({
    e,
    label: 'Previous',
    callback: () => {
      mopidyActions.previous();
      uiActions.createNotification({ content: 'skip_previous', type: 'shortcut' });
    },
  }));

  useHotkeys('.', (e) => prepare({
    e,
    label: 'Next',
    callback: () => {
      mopidyActions.next();
      uiActions.createNotification({ content: 'skip_next', type: 'shortcut' });
    },
  }));

  useHotkeys('=,1+=,2+=,3+=,4+=,5+=', (e, handler) => prepare({
    e,
    label: 'Volume up',
    callback: () => {
      if (handler.key === '=') {
        if (volume !== 'false') {
          uiActions.createNotification({ content: 'volume_up', title: 'Master', type: 'shortcut' });
          let nextVolume = volume + 5;
          if (nextVolume > 100) nextVolume = 100;
          mopidyActions.setVolume(nextVolume);
          if (mute) {
            mopidyActions.setMute(false);
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
          uiActions.createNotification({ content: 'volume_down', title: 'Master', type: 'shortcut' });
          let nextVolume = volume - 5;
          if (nextVolume < 0) nextVolume = 0;
          mopidyActions.setVolume(nextVolume);
          if (mute) {
            mopidyActions.setMute(false);
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
          mopidyActions.setMute(false);
          uiActions.createNotification({ content: 'volume_up', title: 'Master', type: 'shortcut' });
        } else {
          mopidyActions.setMute(true);
          uiActions.createNotification({ content: 'volume_off', title: 'Master', type: 'shortcut' });
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
        uiActions.dragEnd();
        e.preventDefault();
      } else if ($('body').hasClass('modal-open')) {
        window.history.back();
        e.preventDefault();
      }
    },
  }), {}, [dragging]);

  return null;
}

const mapStateToProps = (state) => ({
  volume: state.mopidy.volume,
  mute: state.mopidy.mute,
  play_state: state.mopidy.play_state,
  play_time_position: parseInt(state.mopidy.time_position, 10),
  dragging: state.ui.dragger && state.ui.dragger.dragging,
  allow_reporting: state.ui.allow_reporting,
  snapcast_groups: state.snapcast.groups,
  snapcast_clients: state.snapcast.clients,
  show_disconnected_clients: state.ui.snapcast_show_disconnected_clients || false,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  snapcastActions: bindActionCreators(snapcastActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Hotkeys);

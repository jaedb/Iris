import React from 'react';
import ReactGA from 'react-ga';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';

class Hotkeys extends React.Component {
  constructor(props) {
    super(props);
    this.handleKeyDown = this.handleKeyDown.bind(this);
  }

  componentDidMount() {
    window.addEventListener('keydown', this.handleKeyDown, false);
  }

  componentWillUnmount() {
    window.removeEventListener('keydown', this.handleKeyDown, false);
  }

  handleKeyDown(e) {
    const {
      play_state,
      mopidyActions,
      uiActions,
      mute,
      play_time_position,
      history,
      dragging,
      allow_reporting,
    } = this.props;
    let { volume } = this.props;
    let { key } = e;
    const {
      target,
      altKey,
      ctrlKey,
      metaKey,
      shiftKey,
    } = e;
    key = key.toLowerCase();

    // Ignore text input fields
    if (
      (target.nodeName === 'INPUT' && (target.type === 'text' || target.type === 'number'))
      || target.nodeName === 'TEXTAREA'
      || (target.nodeName === 'BUTTON' && key === ' ')) {
      return;
    }

    // Ignore when there are any key modifiers. This enables us to avoid interfering
    // with browser- and OS-default functions.
    if (altKey || ctrlKey || metaKey || shiftKey) {
      return;
    }

    switch (key) {
      case 'i': {
        history.push('/hotkeys');
        e.preventDefault();
        if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Hotkey info' });
        break;
      }

      case ' ':
      case 'p': // Super-useful once you get used to it. This negates the issue where interactive elements
        // are in focus (ie slider) and <space> is reserved for that field's interactivity.
        if (play_state === 'playing') {
          mopidyActions.pause();
          uiActions.createNotification({ content: 'pause', type: 'shortcut' });
          if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Pause' });
        } else {
          mopidyActions.play();
          uiActions.createNotification({ content: 'play_arrow', type: 'shortcut' });
          if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Play' });
        }
        e.preventDefault();
        break;

      // Stop
      case 's': {
        mopidyActions.stop();
        uiActions.createNotification({ content: 'stop', type: 'shortcut' });
        e.preventDefault();
        if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Stop' });
        break;
      }
      // Seek backwards 30 sec
      case 'r': {
        let new_position = play_time_position - 30000;
        if (new_position < 0) {
          new_position = 0;
        }
        mopidyActions.setTimePosition(new_position);
        uiActions.createNotification({ content: 'fast_rewind', type: 'shortcut' });
        e.preventDefault();
        if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Rewind' });
        break;
      }
      // Seek forwards 30 sec
      case 'f':
        mopidyActions.setTimePosition(play_time_position + 30000);
        uiActions.createNotification({ content: 'fast_forward', type: 'shortcut' });
        e.preventDefault();
        if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Fastforward' });
        break;

      case ',':
      case '<': // Previous track
        mopidyActions.previous();
        uiActions.createNotification({ content: 'skip_previous', type: 'shortcut' });
        e.preventDefault();
        if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Previous' });
        break;

      case '.':
      case '>': // Next track
        mopidyActions.next();
        uiActions.createNotification({ content: 'skip_next', type: 'shortcut' });
        e.preventDefault();
        if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Next' });
        break;

      case '=':
      case '+': // Volume up
        if (volume !== 'false') {
          volume += 5;
          if (volume > 100) {
            volume = 100;
          }
          mopidyActions.setVolume(volume);
          if (mute) {
            mopidyActions.setMute(false);
          }
          uiActions.createNotification({ content: 'volume_up', type: 'shortcut' });
        }
        e.preventDefault();
        if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Volume up' });
        break;

      case '-': // Volume down
        if (volume !== 'false') {
          volume -= 5;
          if (volume < 0) {
            volume = 0;
          }
          mopidyActions.setVolume(volume);
          if (mute) {
            mopidyActions.setMute(false);
          }
        }
        uiActions.createNotification({ content: 'volume_down', type: 'shortcut' });
        e.preventDefault();
        if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Volume down' });
        break;

      case '0': // Mute
        if (mute) {
          mopidyActions.setMute(false);
          uiActions.createNotification({ content: 'volume_up', type: 'shortcut' });
          if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Unmute' });
        } else {
          mopidyActions.setMute(true);
          uiActions.createNotification({ content: 'volume_off', type: 'shortcut' });
          if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Mute' });
        }
        e.preventDefault();
        break;

      case 'escape': // Cancel current action/context
        if (dragging) {
          uiActions.dragEnd();
          e.preventDefault();
          if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Dragging' });
        } else if ($('body').hasClass('modal-open')) {
          window.history.back();
          e.preventDefault();
          if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Modal' });
        }
        break;

      case '1': // Navigation: Queue
        history.push('/queue');
        e.preventDefault();
        if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Queue' });
        break;

      case '2': // Navigation: Search
        history.push('/search');
        e.preventDefault();
        if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Search' });
        break;

      case '3': // Navigation: Kiosk mode
        history.push('/kiosk-mode');
        e.preventDefault();
        if (allow_reporting) ReactGA.event({ category: 'Hotkey', action: key, label: 'Kiosk mode' });
        break;

      default:
        break;
    }
  }

  render() {
    return null;
  }
}

const mapStateToProps = (state) => ({
  volume: state.mopidy.volume,
  mute: state.mopidy.mute,
  play_state: state.mopidy.play_state,
  play_time_position: parseInt(state.mopidy.time_position, 10),
  dragging: state.ui.dragger && state.ui.dragger.dragging,
  allow_reporting: state.ui.allow_reporting,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Hotkeys);

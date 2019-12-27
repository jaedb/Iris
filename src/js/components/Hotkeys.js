import React from 'react';
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
      modal,
      dragging,
    } = this.props;
    let { volume } = this.props;
    const key = e.key.toLowerCase();

    // Ignore text input fields
    if (
      (e.target.nodeName === 'INPUT' && (e.target.type === 'text' || e.target.type === 'number')) ||
      e.target.nodeName === 'TEXTAREA' ||
      (e.target.nodeName === 'BUTTON' && key === ' ')) {
      return;
    }

    // Ignore when there are any key modifiers. This enables us to avoid interfering
    // with browser- and OS-default functions.
    if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) {
      return;
    }

    let prevent = false;
    switch (key) {
      case ' ':
      case 'p': // Super-useful once you get used to it. This negates the issue where interactive elements
        // are in focus (ie slider) and <space> is reserved for that field's interactivity.
        if (play_state == 'playing') {
          mopidyActions.pause();
          uiActions.createNotification({ content: 'pause', type: 'shortcut' });
        } else {
          mopidyActions.play();
          uiActions.createNotification({ content: 'play_arrow', type: 'shortcut' });
        }
        prevent = true;
        break;

      case 'escape':
        if (dragging) {
          uiActions.dragEnd();
          prevent = true;
        } else if (modal) {
          window.history.back();
          prevent = true;
        }
        break;

      case 's':
        history.push('/search');
        prevent = true;
        break;

      case 'q':
        history.push('/queue');
        prevent = true;
        break;

      case 'k':
        history.push('/kiosk-mode');
        prevent = true;
        break;

      case ',':
        window.history.back();
        prevent = true;
        break;

      case '.':
        window.history.forward();
        prevent = true;
        break;

      case '=':
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
        prevent = true;
        break;

      case '-':
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
        prevent = true;
        break;

      case '0':
        if (mute) {
          mopidyActions.setMute(false);
          uiActions.createNotification({ content: 'volume_up', type: 'shortcut' });
        } else {
          mopidyActions.setMute(true);
          uiActions.createNotification({ content: 'volume_off', type: 'shortcut' });
        }
        prevent = true;
        break;

      case ';':
        var new_position = play_time_position - 30000;
        if (new_position < 0) {
          new_position = 0;
        }
        mopidyActions.setTimePosition(new_position);
        uiActions.createNotification({ content: 'fast_rewind', type: 'shortcut' });
        prevent = true;
        break;

      case "'":
        mopidyActions.setTimePosition(play_time_position + 30000);
        uiActions.createNotification({ content: 'fast_forward', type: 'shortcut' });
        prevent = true;
        break;

      case '[':
        mopidyActions.previous();
        uiActions.createNotification({ content: 'skip_previous', type: 'shortcut' });
        prevent = true;
        break;

      case ']':
        mopidyActions.next();
        uiActions.createNotification({ content: 'skip_next', type: 'shortcut' });
        prevent = true;
        break;

      default:
        break;
    }

    if (prevent) {
      e.preventDefault();
    }
  }

  render() {
    return null;
  }
}

const mapStateToProps = (state, ownProps) => ({
  volume: (state.mopidy.volume ? state.mopidy.volume : false),
  mute: state.mopidy.mute,
  play_state: state.mopidy.play_state,
  play_time_position: parseInt(state.mopidy.time_position),
  dragging: state.ui.dragger && state.ui.dragger.dragging,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Hotkeys);

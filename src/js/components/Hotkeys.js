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

  componentWillMount() {
    window.addEventListener('keydown', this.handleKeyDown, false);
  }

  componentWillUnmount() {
    window.removeEventListener('keydown', this.handleKeyDown, false);
  }

  handleKeyDown(e) {
    // When we're focussed on certian elements, don't fire any shortcuts
    // Typically form inputs
    const ignoreNodes = ['INPUT', 'TEXTAREA', 'BUTTON'];
    if (ignoreNodes.indexOf(e.target.nodeName) > -1) {
      return;
    }

    // Ignore when there are any key modifiers. This enables us to avoid interfering
    // with browser- and OS-default functions.
    if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) {
      return;
    }

    let prevent = false;
    switch (e.key.toLowerCase()) {
      case ' ':
      case 'p': // Super-useful once you get used to it. This negates the issue where interactive elements
        // are in focus (ie slider) and <space> is reserved for that field's interactivity.
        if (this.props.play_state == 'playing') {
          this.props.mopidyActions.pause();
          this.props.uiActions.createNotification({ content: 'pause', type: 'shortcut' });
        } else {
          this.props.mopidyActions.play();
          this.props.uiActions.createNotification({ content: 'play_arrow', type: 'shortcut' });
        }
        prevent = true;
        break;

      case 'escape':
        if (this.props.dragging) {
          this.props.uiActions.dragEnd();
          prevent = true;
        } else if (this.props.modal) {
			        window.history.back();
          prevent = true;
        }
        break;

      case 's':
        this.props.history.push('/search');
        prevent = true;
        break;

      case 'q':
        this.props.history.push('/queue');
        prevent = true;
        break;

      case 'k':
        this.props.history.push('/kiosk-mode');
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
        var { volume } = this.props;
        if (volume !== 'false') {
          volume += 5;
          if (volume > 100) {
            volume = 100;
          }
          this.props.mopidyActions.setVolume(volume);
          if (this.props.mute) {
            this.props.mopidyActions.setMute(false);
          }
          this.props.uiActions.createNotification({ content: 'volume_up', type: 'shortcut' });
        }
        prevent = true;
        break;

      case '-':
        var { volume } = this.props;
        if (volume !== 'false') {
          volume -= 5;
          if (volume < 0) {
            volume = 0;
          }
          this.props.mopidyActions.setVolume(volume);
          if (this.props.mute) {
            this.props.mopidyActions.setMute(false);
          }
        }
        this.props.uiActions.createNotification({ content: 'volume_down', type: 'shortcut' });
        prevent = true;
        break;

      case '0':
        if (this.props.mute) {
          this.props.mopidyActions.setMute(false);
          this.props.uiActions.createNotification({ content: 'volume_up', type: 'shortcut' });
        } else {
          this.props.mopidyActions.setMute(true);
          this.props.uiActions.createNotification({ content: 'volume_off', type: 'shortcut' });
        }
        prevent = true;
        break;

      case ';':
        var new_position = this.props.play_time_position - 30000;
        if (new_position < 0) {
          new_position = 0;
        }
        this.props.mopidyActions.setTimePosition(new_position);
        this.props.uiActions.createNotification({ content: 'fast_rewind', type: 'shortcut' });
        prevent = true;
        break;

      case "'":
        this.props.mopidyActions.setTimePosition(this.props.play_time_position + 30000);
        this.props.uiActions.createNotification({ content: 'fast_forward', type: 'shortcut' });
        prevent = true;
        break;

      case '[':
        this.props.mopidyActions.previous();
        this.props.uiActions.createNotification({ content: 'skip_previous', type: 'shortcut' });
        prevent = true;
        break;

      case ']':
        this.props.mopidyActions.next();
        this.props.uiActions.createNotification({ content: 'skip_next', type: 'shortcut' });
        prevent = true;
        break;
    }

    if (prevent) {
      e.preventDefault();
      return false;
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

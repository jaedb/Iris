
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import Link from './Link';
import ProgressSlider from './Fields/ProgressSlider';
import VolumeControl from './Fields/VolumeControl';
import MuteControl from './Fields/MuteControl';
import OutputControl from './Fields/OutputControl';
import { Dater } from './Dater';
import LinksSentence from './LinksSentence';
import Thumbnail from './Thumbnail';
import Icon from './Icon';
import { scrollTo } from '../util/helpers';
import * as uiActions from '../services/ui/actions';
import * as coreActions from '../services/core/actions';
import * as mopidyActions from '../services/mopidy/actions';
import { I18n } from '../locale';

class PlaybackControls extends React.Component {
  constructor(props) {
    super(props);
    this.stream = null;
    this.state = {
      expanded: false,
      current_track: null,
      transition_track: null,
      transition_direction: null,
    };
  }

  static getDerivedStateFromProps({ current_track }, state) {
    return {
      ...state,
      current_track,
    };
  }

  setTransition(direction) {
    this.setState({
      transition_track: this.state.current_track,
      transition_direction: direction,
    });

    // Allow time for the animation to complete, then remove
    // the transitioning track from state
    setTimeout(() => {
      this.setState({
        transition_track: null,
      });
    },
    250);
  }

  handleContextMenu = (e) => {
    const {
      current_track,
    } = this.state;
    const {
      uiActions: {
        showContextMenu,
      },
    } = this.props;

    e.preventDefault();

    showContextMenu({
      e,
      context: 'current-track',
      items: [current_track],
      uris: [current_track.uri],
    });
  }

  handleTouchStart = (e) => {
    const { touch_enabled } = this.props;
    if (!touch_enabled) return;

    const timestamp = Math.floor(Date.now());

    // Save touch start details
    this.start_time = timestamp;
    this.start_position = {
      x: e.touches[0].clientX,
    };

    return false;
  }

  handleTouchEnd = (e) => {
    const { touch_enabled } = this.props;
    if (!touch_enabled) return;

    const timestamp = Math.floor(Date.now());
    const tap_distance_threshold = 10;		// Max distance (px) between touchstart and touchend to qualify as a tap
    const tap_time_threshold = 200;			// Max time (ms) between touchstart and touchend to qualify as a tap
    const end_position = {
      x: e.changedTouches[0].clientX,
    };

    // Too long between touchstart and touchend
    if (this.start_time + tap_time_threshold < timestamp) {
      return false;
    }

    // Make sure there's enough distance between start and end before we handle
    // this event as a 'tap'
    if (this.start_position.x + tap_distance_threshold > end_position.x
			&& this.start_position.x - tap_distance_threshold < end_position.x) {
      // Scroll to top (without smooth_scroll)
      scrollTo(null, false);
      this.props.history.push('/queue');
    } else {
      // Swipe to the left = previous track
      if (this.start_position.x < end_position.x) {
        this.setTransition('previous');
        this.props.mopidyActions.previous();

        // Swipe to the right = skip track
      } else if (this.start_position.x > end_position.x) {
        this.setTransition('next');
        this.props.mopidyActions.next();
      }
    }

    this.end_time = timestamp;
    e.preventDefault();
  }

  renderPlayButton() {
    let button = <button className="control play" onClick={() => this.props.mopidyActions.play()}><Icon name="play_circle_filled" type="material" /></button>;
    if (this.props.play_state == 'playing') {
      button = <button className="control play" onClick={() => this.props.mopidyActions.pause()}><Icon name="pause_circle_filled" type="material" /></button>;
    }
    return button;
  }

  renderConsumeButton() {
    let button = (
      <button className="control tooltip" onClick={() => this.props.mopidyActions.setConsume(true)}>
        <Icon name="restaurant" type="material" />
        <span className="tooltip__content">
          <I18n path="playback_controls.consume" />
        </span>
      </button>
    );
    if (this.props.consume) {
      button = (
        <button className="control control--active tooltip" onClick={() => this.props.mopidyActions.setConsume(false)}>
          <Icon name="restaurant" type="material" />
          <span className="tooltip__content">
            <I18n path="playback_controls.consume" />
          </span>
        </button>
      );
    }
    return button;
  }

  renderRandomButton() {
    let button = (
      <button className="control tooltip" onClick={() => this.props.mopidyActions.setRandom(true)}>
        <Icon name="shuffle" type="material" />
        <span className="tooltip__content">
          <I18n path="playback_controls.shuffle" />
        </span>
      </button>
    );
    if (this.props.random) {
      button = (
        <button className="control control--active tooltip" onClick={() => this.props.mopidyActions.setRandom(false)}>
          <Icon name="shuffle" type="material" />
          <span className="tooltip__content">
            <I18n path="playback_controls.shuffle" />
          </span>
        </button>
      );
    }
    return button;
  }

  renderRepeatButton() {
    let button = (
      <button className="control tooltip" onClick={() => this.props.mopidyActions.setRepeat(true)}>
        <Icon name="repeat" />
        <span className="tooltip__content">
          <I18n path="playback_controls.repeat" />
        </span>
      </button>
    );
    if (this.props.repeat) {
      button = (
        <button className="control control--active tooltip" onClick={() => this.props.mopidyActions.setRepeat(false)}>
          <Icon name="repeat" />
          <span className="tooltip__content">
            <I18n path="playback_controls.repeat" />
          </span>
        </button>
      );
    }
    return button;
  }

  render() {
    const {
      next_track,
      touch_enabled,
      time_position,
    } = this.props;
    const {
      current_track,
      expanded,
      transition_track,
      transition_direction,
    } = this.state;

    return (
      <div className={`playback-controls${expanded ? ' playback-controls--expanded' : ''}${touch_enabled ? ' playback-controls--touch-enabled' : ''}`}>

        <div className="playback-controls__background" />

        {next_track && next_track.images ? <Thumbnail className="hide" size="large" images={next_track.images} /> : null}

        <div
          className="current-track__wrapper"
          transition={transition_track}
          direction={transition_direction}
        >
          {transition_track && transition_direction && (
            <div className={`current-track current-track__outgoing`}>
              <div className="text">
                <div className="title">
                  {transition_track.name}
                </div>
                <div className="artist">
                  <LinksSentence items={transition_track.artists} nolinks />
                </div>
              </div>
            </div>
          )}

          {current_track && (!transition_track || transition_track.tlid !== current_track.tlid) ? (
            <div
              className="current-track current-track__incoming"
              onTouchStart={this.handleTouchStart}
              onTouchEnd={this.handleTouchEnd}
              onContextMenu={this.handleContextMenu}
              tabIndex="-1"
              key={current_track.tlid}
            >
              <Link className="thumbnail-wrapper" to="/kiosk-mode" tabIndex="-1">
                <Thumbnail size="small" images={current_track.images} type="track" />
              </Link>
              <div className="text">
                <div className="title">
                  {current_track ? current_track.name : <span>-</span>}
                </div>
                <div className="artist">
                  {current_track ? <LinksSentence items={current_track.artists} /> : <LinksSentence />}
                </div>
              </div>
            </div>
          ) : (
            <div
              className="current-track"
              onTouchStart={this.handleTouchStart}
              onTouchEnd={this.handleTouchEnd}
              tabIndex="-1"
            >
              <Link className="thumbnail-wrapper" to="/kiosk-mode" tabIndex="-1">
                <Thumbnail size="small" type="track" />
              </Link>
              <div className="text">
                <div className="title">&nbsp;</div>
                <div className="artist">&nbsp;</div>
              </div>
            </div>
          )}
        </div>

        <section className="playback">
          <button className="control previous" onClick={() => this.props.mopidyActions.previous()}>
            <Icon name="navigate_before" type="material" />
          </button>
          { this.renderPlayButton() }
          <button className="control next" onClick={() => this.props.mopidyActions.next()}>
            <Icon name="navigate_next" type="material" />
          </button>
        </section>

        <section className="progress">
          <div className="time time--current">
            {time_position ? <Dater type="length" data={time_position} /> : '-'}
          </div>
          <ProgressSlider />
          <div className="time time--total">
            {current_track ? <Dater type="length" data={current_track.duration} /> : '-'}
          </div>
        </section>

        <section className="settings">
          {this.renderConsumeButton()}
          {this.renderRandomButton()}
          {this.renderRepeatButton()}
          <OutputControl force_expanded={this.state.expanded} />
        </section>

        <section className="volume">
          <MuteControl
            mute={this.props.mute}
            onMuteChange={(mute) => this.props.mopidyActions.setMute(mute)}
          />
          <VolumeControl
            scrollWheel
            volume={this.props.volume}
            mute={this.props.mute}
            onVolumeChange={(percent) => this.props.mopidyActions.setVolume(percent)}
          />
        </section>

        <section className="triggers">
          <button className="control expanded-controls" onClick={(e) => this.setState({ expanded: !this.state.expanded })}>
            {this.state.expanded ? <Icon name="expand_more" type="material" /> : <Icon name="expand_less" type="material" />}
          </button>
          <button className={`control sidebar-toggle${this.props.sidebar_open ? ' open' : ''}`} onClick={(e) => this.props.uiActions.toggleSidebar()}>
            <Icon className="open" name="menu" type="material" />
          </button>
        </section>

      </div>
    );
  }
}

const mapStateToProps = (state) => {
  const {
    items,
    next_track_uri,
    current_track,
  } = state.core;

  return {
    snapcast_enabled: state.pusher.config.snapcast_enabled,
    current_track: current_track ? items[current_track.uri] || current_track : null,
    next_track: items[next_track_uri],
    radio_enabled: (!!(state.ui.radio && state.ui.radio.enabled)),
    play_state: state.mopidy.play_state,
    time_position: state.mopidy.time_position,
    consume: state.mopidy.consume,
    repeat: state.mopidy.repeat,
    random: state.mopidy.random,
    volume: state.mopidy.volume,
    mute: state.mopidy.mute,
    sidebar_open: state.ui.sidebar_open,
    slim_mode: state.ui.slim_mode,
    touch_enabled: state.ui.playback_controls_touch_enabled,
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(PlaybackControls);

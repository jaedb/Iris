import React, { useState } from 'react';
import { useDispatch, useSelector } from 'react-redux';
import { useHistory } from 'react-router-dom';
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
import { showContextMenu, toggleSidebar } from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import { I18n } from '../locale';
import { makeItemSelector } from '../util/selectors';
import { formatSimpleObject } from '../util/format';

const PlaybackControls = () => {
  const [expanded, setExpanded] = useState();
  const [transitionTrack, setTransitionTrack] = useState();
  const [transitionDirection, setTransitionDirection] = useState();
  const [touchMeta, setTouchMeta] = useState({});

  const history = useHistory();
  const dispatch = useDispatch();
  const touch_enabled = useSelector((state) => state.ui.touch_enabled);
  const sidebar_open = useSelector((state) => state.ui.sidebar_open);
  const volume = useSelector((state) => state.mopidy.volume);
  const mute = useSelector((state) => state.mopidy.mute);
  const time_position = useSelector((state) => state.mopidy.time_position);
  const play_state = useSelector((state) => state.mopidy.play_state);
  const consume = useSelector((state) => state.mopidy.consume);
  const random = useSelector((state) => state.mopidy.random);
  const repeat = useSelector((state) => state.mopidy.repeat);
  const streamTitle = useSelector((state) => state.core.streamTitle);
  const currentTrackUri = useSelector((state) => state.core.current_track?.uri);
  const currentTrackSelector = makeItemSelector(currentTrackUri);
  const currentTrack = useSelector(currentTrackSelector);
  const nextTrackUri = useSelector((state) => state.core.next_track_uri);
  const nextTrackSelector = makeItemSelector(nextTrackUri);
  const nextTrack = useSelector(nextTrackSelector);

  const setTransition = (direction) => {
    setTransitionTrack(currentTrack);
    setTransitionDirection(direction);

    // Allow time for the animation to complete, then remove
    // the transitioning track from state
    setTimeout(() => {
      setTransitionTrack(null);
    },
    250);
  };

  const handleContextMenu = (e) => {
    e.preventDefault();

    dispatch(
      showContextMenu({
        e,
        source: {
          ...formatSimpleObject(currentTrack),
          context: 'current-track',
        },
        items: [currentTrack],
        uris: [currentTrack.uri],
      }),
    );
  };

  const handleTouchStart = (e) => {
    if (!touch_enabled) return;

    const timestamp = Math.floor(Date.now());

    // Save touch start details
    setTouchMeta({
      start_time: timestamp,
      start_position: {
        x: e.touches[0].clientX,
      },
    });

    return false;
  };

  const handleTouchEnd = (e) => {
    if (!touch_enabled) return;

    const timestamp = Math.floor(Date.now());
    const tap_distance_threshold = 10; // Max distance (px) between touchstart and touchend to qualify as a tap
    const tap_time_threshold = 200; // Max time (ms) between touchstart and touchend to qualify as a tap
    const end_position = {
      x: e.changedTouches[0].clientX,
    };

    // Too long between touchstart and touchend
    if (touchMeta.start_time + tap_time_threshold < timestamp) {
      return false;
    }

    // Make sure there's enough distance between start and end before we handle
    // this event as a 'tap'
    if (
      touchMeta.start_position.x + tap_distance_threshold > end_position.x
      && touchMeta.start_position.x - tap_distance_threshold < end_position.x
    ) {
      // Scroll to top (without smooth_scroll)
      scrollTo(null, false);
      history.push('/queue');
    } else {
      // Swipe to the left = previous track
      if (touchMeta.start_position.x < end_position.x) {
        setTransition('previous');
        dispatch(mopidyActions.previous());

        // Swipe to the right = skip track
      } else if (touchMeta.start_position.x > end_position.x) {
        setTransition('next');
        dispatch(mopidyActions.next());
      }
    }

    setTouchMeta((prev) => ({ ...prev, end_time: timestamp }));
    e.preventDefault();
  };

  return (
    <div className={`playback-controls${expanded ? ' playback-controls--expanded' : ''}${touch_enabled ? ' playback-controls--touch-enabled' : ''}`}>

      <div className="playback-controls__background" />

      {nextTrack && nextTrack.images ? <Thumbnail className="hide" size="large" images={nextTrack.images} /> : null}

      <div
        className="current-track__wrapper"
        transition={transitionTrack}
        direction={transitionDirection}
      >
        {transitionTrack && transitionDirection && (
          <div className="current-track current-track__outgoing">
            <div className="text">
              <div className="title">
                {transitionTrack.name}
              </div>
              <div className="artist">
                <LinksSentence items={transitionTrack.artists} type="artist" nolinks />
              </div>
            </div>
          </div>
        )}

        {currentTrack && (!transitionTrack || transitionTrack.tlid !== currentTrack.tlid) ? (
          <div
            className="current-track current-track__incoming"
            onTouchStart={handleTouchStart}
            onTouchEnd={handleTouchEnd}
            onContextMenu={handleContextMenu}
            tabIndex="-1"
            key={currentTrack.tlid}
          >
            <Link className="thumbnail-wrapper" to="/modal/kiosk-mode" tabIndex="-1">
              <Thumbnail size="small" images={currentTrack.images} type="track" />
            </Link>
            <div className="text">
              <div className="title">
                {streamTitle && <span>{streamTitle}</span>}
                {!streamTitle && currentTrack && <span>{currentTrack.name}</span>}
                {!streamTitle && !currentTrack && <span>-</span>}
              </div>
              <div className="artist">
                {
                  (currentTrack && currentTrack.artists
                      && <LinksSentence items={currentTrack.artists} type="artist" />)
                  || (streamTitle && <span className="links-sentence">{streamTitle}</span>)
                  || <LinksSentence />
                }
              </div>
            </div>
          </div>
        ) : (
          <div
            className="current-track"
            onTouchStart={handleTouchStart}
            onTouchEnd={handleTouchEnd}
            tabIndex="-1"
          >
            <Link className="thumbnail-wrapper" to="/modal/kiosk-mode" tabIndex="-1">
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
        <button
          type="button"
          className="control previous"
          onClick={() => dispatch(mopidyActions.previous())}
        >
          <Icon name="navigate_before" type="material" />
        </button>
        {
          play_state === 'playing' ? (
            <button
              type="button"
              className="control play"
              onClick={() => dispatch(mopidyActions.pause())}
            >
              <Icon name="pause_circle_filled" type="material" />
            </button>
          ) : (
            <button
              type="button"
              className="control play"
              onClick={() => dispatch(mopidyActions.play())}
            >
              <Icon name="play_circle_filled" type="material" />
            </button>
          )
        }
        <button
          type="button"
          className="control next"
          onClick={() => dispatch(mopidyActions.next())}
        >
          <Icon name="navigate_next" type="material" />
        </button>
      </section>

      <section className="progress">
        <div className="time time--current">
          {time_position ? <Dater type="length" data={time_position} /> : '-'}
        </div>
        <ProgressSlider />
        <div className="time time--total">
          {currentTrack ? <Dater type="length" data={currentTrack.duration} /> : '-'}
        </div>
      </section>

      <section className="settings">
        <button
          type="button"
          className={`control${consume ? ' control--active' : ''} tooltip`}
          onClick={() => dispatch(mopidyActions.setConsume(!consume))}
        >
          <Icon name="restaurant" type="material" />
          <span className="tooltip__content">
            <I18n path="playback_controls.consume" />
          </span>
        </button>
        <button
          type="button"
          className={`control${random ? ' control--active' : ''} tooltip`}
          onClick={() => dispatch(mopidyActions.setRandom(!random))}
        >
          <Icon name="shuffle" type="material" />
          <span className="tooltip__content">
            <I18n path="playback_controls.shuffle" />
          </span>
        </button>
        <button
          type="button"
          className={`control${repeat ? ' control--active' : ''} tooltip`}
          onClick={() => dispatch(mopidyActions.setRepeat(!repeat))}
        >
          <Icon name="repeat" type="material" />
          <span className="tooltip__content">
            <I18n path="playback_controls.repeat" />
          </span>
        </button>
        <OutputControl force_expanded={expanded} />
      </section>

      <section className="volume">
        <MuteControl
          mute={mute}
          onMuteChange={(value) => dispatch(mopidyActions.setMute(value))}
        />
        <VolumeControl
          scrollWheel
          volume={volume}
          mute={mute}
          onVolumeChange={(value) => dispatch(mopidyActions.setVolume(value))}
        />
      </section>

      <section className="triggers">
        <button
          type="button"
          className="control expanded-controls"
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? (
            <Icon name="expand_more" type="material" />
          ) : (
            <Icon name="expand_less" type="material" />
          )}
        </button>
        <button
          type="button"
          className={`control sidebar-toggle${sidebar_open ? ' open' : ''}`}
          onClick={() => dispatch(toggleSidebar())}
        >
          <Icon className="open" name="menu" type="material" />
        </button>
      </section>

    </div>
  );
}

export default PlaybackControls;

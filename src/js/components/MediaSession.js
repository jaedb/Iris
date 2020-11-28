import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import * as mopidyActions from '../services/mopidy/actions';

class MediaSession extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      current_track: null,
      stream_title: null,
    };
  }

  componentDidMount() {
    const {
      mediaSession,
    } = navigator;

    mediaSession.setActionHandler('play', () => this.actionHandler('play'));
    mediaSession.setActionHandler('pause', () => this.actionHandler('pause'));
    mediaSession.setActionHandler('seekbackward', () => this.actionHandler('seekbackward'));
    mediaSession.setActionHandler('seekforward', () => this.actionHandler('seekforward'));
    mediaSession.setActionHandler('previoustrack', () => this.actionHandler('previous'));
    mediaSession.setActionHandler('nexttrack', () => this.actionHandler('next'));
  }

  static getDerivedStateFromProps({ current_track, stream_title, play_state }, state) {
    if (current_track) {
      navigator.mediaSession.metadata = new window.MediaMetadata({
        title: stream_title || current_track.title,
        artist: current_track.artists[0].name,
        album: current_track.album.name,
        artwork: [
          {
            src: current_track.images ? current_track.images.small : '',
            sizes: '96x96',
            type: 'image/png',
          },
          {
            src: current_track.images ? current_track.images.medium : '',
            sizes: '256x256',
            type: 'image/png',
          },
          {
            src: current_track.images ? current_track.images.huge : '',
            sizes: '512x512',
            type: 'image/png',
          },
        ],
      });
    }

    navigator.mediaSession.playbackState = play_state;

    return {
      ...state,
      current_track,
      stream_title,
    };
  }

  actionHandler = (action) => {
    const {
      mopidyActions: actions,
      time_position,
    } = this.props;

    switch (action) {
      case 'seekbackward': {
        let newposition = time_position - 30000; // 30 seconds
        if (newposition <= 0) newposition = 0;
        return actions.setTimePosition(newposition);
      }
      case 'seekforward': {
        return actions.setTimePosition(time_position + 30000); // 30 seconds
      }
      default: {
        return actions[action]();
      }
    }
  }

  render = () => {
    return (
      // eslint-disable-next-line jsx-a11y/media-has-caption
      <audio
        id="media-session"
        src="/iris/assets/silence.mp3"
        autoPlay
        loop
        style={{ display: 'none' }}
      />
    );
  };
}

const mapStateToProps = (state) => {
  const {
    items,
    current_track,
    stream_title,
  } = state.core;

  return {
    current_track: current_track ? items[current_track.uri] || current_track : null,
    stream_title,
    play_state: state.mopidy.play_state,
    time_position: state.mopidy.time_position,
    volume: state.mopidy.volume,
    mute: state.mopidy.mute,
  };
};

const mapDispatchToProps = (dispatch) => ({
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(MediaSession);

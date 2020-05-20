
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import * as coreActions from '../services/core/actions';

class Stream extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      uri: '',
      play_state: '',
      cachebuster: `${Date.now()}`,
      url: null,
    };

    this.audio = new Audio();
    this.audio.onerror = (error) => this.onError(error);
  }

  static getDerivedStateFromProps(props, state) {
    const {
      volume,
      mute,
      enabled: propEnabled,
      play_state: propPlayState,
      url: propUrl,
      current_track: {
        uri: propUri,
      } = {},
    } = props;
    const {
      play_state: statePlayState,
      enabled: stateEnabled,
      uri: stateUri,
      url: stateUrl,
      cachebuster,
    } = state;

    // Same track as before, and still playing
    if (
      propUri
      && propUri === stateUri
      && propPlayState === statePlayState
      && propEnabled === stateEnabled
      && propUrl === stateUrl
    ) {
      return null;
    }

    let fullUrl = null;
    if (propEnabled && propUrl && propUri) {
      fullUrl = `${propUrl}?cb=${cachebuster}_${propUri}`;
    }

    return {
      ...state,
      enabled: propEnabled,
      uri: propUri,
      play_state: propPlayState,
      volume,
      mute,
      fullUrl,
    };
  }

  componentDidUpdate = (prevProps, prevState) => {
    const {
      fullUrl,
      volume,
    } = this.state;
    const { enabled, mute } = this.props;

    this.audio.muted = mute;
    this.audio.volume = volume ? (volume / 100) : 0.5;

    // Only update URL if it's changed. This prevents re-loading the stream when something unrelated
    // (like volume) was changed
    if ((prevProps.enabled && !enabled)) {
      this.stop();
    } else if (fullUrl && ((prevState.fullUrl !== fullUrl) || (!prevProps.enabled && enabled))) {
      this.play(fullUrl);
    }
  }

  onError = (error) => {
    const { enabled, play_state } = this.props;
    if (!enabled || play_state !== 'playing') return;

    console.error('Audio failed to load, retrying...', error);
    setTimeout(
      () => this.play(),
      250,
    );
  }

  play = (url = null) => {
    const { fullUrl } = this.state;
    console.info(`Playing stream: ${url || fullUrl}`);
    this.audio.src = url || fullUrl;
    this.audio.play();
  }

  stop = () => {
    console.info('Stopping stream');
    this.audio.pause();
    this.audio.src = '';
  }

  render = () => null;
}

const mapStateToProps = (state) => ({
  current_track: state.core.current_track || {},
  play_state: state.mopidy.play_state,
  enabled: state.core.http_streaming_enabled,
  mute: state.core.http_streaming_mute || false,
  volume: state.core.http_streaming_volume >= 0 ? state.core.http_streaming_volume : 50,
  url: (state.core.http_streaming_url ? state.core.http_streaming_url : null),
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Stream);

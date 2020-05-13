
import React, { createRef } from 'react';
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

    this.audioRef = createRef();
  }

  static getDerivedStateFromProps(props, state) {
    const {
      volume,
      enabled: propEnabled,
      play_state: propPlayState,
      url: propUrl,
      current_track: {
        uri: propUri,
      } = {},
    } = props;
    const {
      play_state: statePlayState,
      uri: stateUri,
      cachebuster,
    } = state;

    // Same track as before, and still playing
    if (
      propUri
      && propUri === stateUri
      && propPlayState === statePlayState
    ) {
      return null;
    }

    let url = null;
    if (propEnabled && propUrl && propUri) {
      url = `${propUrl}?cb=${cachebuster}_${propUri}`;
      console.log(`Playing stream: ${url}`);
    }

    return {
      ...state,
      enabled: propEnabled,
      uri: propUri,
      play_state: propPlayState,
      volume,
      url,
    };
  }

  componentDidUpdate = () => {
    const { volume } = this.props;
    if (this.audioRef.current) {
      this.audioRef.current.volume = volume / 100;
    }
  }

  render = () => {
    const { url } = this.state;
    if (!url) return null;
    return (
      <div key={url}>
        <audio autoPlay ref={this.audioRef}>
          <source src={url} />
        </audio>
      </div>
    );
  }
}

const mapStateToProps = (state) => ({
  current_track: state.core.current_track || {},
  play_state: state.mopidy.play_state,
  enabled: state.core.http_streaming_enabled,
  volume: state.core.http_streaming_volume >= 0 ? state.core.http_streaming_volume : 50,
  url: (state.core.http_streaming_url ? state.core.http_streaming_url : null),
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Stream);

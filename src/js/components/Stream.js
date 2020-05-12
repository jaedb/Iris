
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
      cachebuster: '',
    };
  }

  static getDerivedStateFromProps({ play_state, current_track: { uri } = {} }, state) {
    if (uri && uri === state.uri && play_state === state.play_state) return null;
    return {
      ...state,
      uri,
      play_state,
      cachebuster: `${Date.now()}`,
    };
  }

  render = () => {
    const {
      play_state,
      enabled,
      volume,
      url,
      current_track: { uri } = {},
    } = this.props;
    const { cachebuster } = this.state;

    if (!uri || !enabled || !url) return null;

    console.debug(`Attempting to play stream ${url}?cb=${cachebuster}`);

    if (play_state !== 'playing') return null;

    return (
      <audio
        autoPlay
        volume={volume / 100} // TODO: Needs to be set by script, not DOM
      >
        <source src={`${url}?cb=${cachebuster}`} />
      </audio>
    );
  }
}

const mapStateToProps = (state) => ({
  current_track: state.core.current_track || {},
  play_state: state.mopidy.play_state,
  enabled: state.core.http_streaming_enabled,
  volume: state.core.http_streaming_volume >= 0 ? state.core.http_streaming_volume : 50,
  url: (state.core.http_streaming_url ? state.core.http_streaming_url : null),
  cachebuster: state.core.http_streaming_cachebuster,
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Stream);

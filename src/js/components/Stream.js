
import React from 'react';
import ReactHowler from 'react-howler';
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
    if (uri === state.uri && play_state === state.play_state) return null;
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
    } = this.props;
    const { cachebuster } = this.state;

    if (!enabled || !url) return null;

    return (
      <ReactHowler
        src={`${url}?cb=${cachebuster}`}
        playing={play_state === 'playing'}
        volume={volume / 100}
        onLoad={() => console.debug(`Loaded stream ${url}?cb=${cachebuster}`)}
        html5
      />
    );
  }
};

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

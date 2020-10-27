
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import * as mopidyActions from '../../services/mopidy/actions';
import { throttle } from '../../util/helpers';

class ProgressSlider extends React.Component {
  constructor(props) {
    super(props);

    this.handleChange = throttle(this.handleChange.bind(this), 250);
  }

  handleChange(value) {
    this.props.mopidyActions.setTimePosition(this.props.current_track.duration * (value / 100));
  }

  render() {
    let percent = 0;
    if (this.props.connected && this.props.time_position && this.props.current_track && this.props.current_track.duration) {
      percent = this.props.time_position / this.props.current_track.duration;
      percent *= 100;
      if (percent > 1000) {
        percent = 100;
      }
    }

    return (
      <div className={`slider slider--playback-progress slider--${this.props.play_state}`}>
        <input
          type="range"
          min="0"
          max="100"
          value={percent}
          className="slider__input"
          onChange={(e) => this.handleChange(parseInt(e.target.value))}
        />
        <div className="slider__track">
          <div className="slider__track__progress" style={{ width: `${percent}%` }} />
        </div>
      </div>
    );
  }
}

/**
 * Export our component
 *
 * We also integrate our global store, using connect()
 * */

const mapStateToProps = (state, ownProps) => ({
  current_track: (state.core.current_track && state.core.items[state.core.current_track.uri] !== undefined ? state.core.items[state.core.current_track.uri] : null),
  connected: state.mopidy.connected,
  time_position: state.mopidy.time_position,
  play_state: state.mopidy.play_state,
});

const mapDispatchToProps = (dispatch) => ({
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(ProgressSlider);

import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import localForage from 'localforage';
import { get as getStorage } from '../util/storage';
import { isTouchDevice } from '../util/helpers';
import * as uiActions from '../services/ui/actions';
import { indexToArray } from '../util/arrays';

class DebugInfo extends React.Component {
  constructor(props) {
    super(props);
    this.state = { localForageLength: 0 };
  }

  componentDidMount() {
    localForage.length().then((localForageLength) => this.setState({ localForageLength }));
  }

  localStorageSize() {
    let data = '';

    for (const key in window.localStorage) {
      if (window.localStorage.hasOwnProperty(key)) {
        data += window.localStorage[key];
      }
    }

    let used = 0;
    const total = 5000;
    if (data !== '') {
      used = ((data.length * 16) / (8 * 1024)).toFixed(2);
    }

    return {
      used,
      percent: (used / total * 100).toFixed(2),
    };
  }

  renderLoadQueue = () => {
    const { load_queue } = this.props;
    if (!load_queue) return <div className="debug-info-item mid_grey-text">Nothing loading</div>;

    const queue = indexToArray(load_queue);

    if (queue.length > 0) {
      return (
        <div className="debug-info-item">
          {queue.map((item, index) => (<div key={`${item}_${index}`}>{item}</div>))}
        </div>
      );
    }
    return <div className="debug-info-item mid_grey-text">Nothing loading</div>;
  }

  render = () => {
    const localStorageUsage = this.localStorageSize();
    const {
      items = {},
      notifications = {},
      processes = {},
      slim_mode,
      test_mode,
      selected_tracks = [],
      enqueue_uris_batches = [],
    } = this.props;

    return (
      <div className="debug-info">

        <div className="debug-info-section">
          <div className="debug-info-item">
            Version:
            {' '}
            {version}
          </div>
          <div className="debug-info-item">
            Build:
            {' '}
            {build}
          </div>
        </div>

        <div className="debug-info-section">
          <h5>State</h5>
          <div className="debug-info-item">
            {`Items: ${Object.keys(items).length}`}
          </div>
          <div className="debug-info-item">
            {`Coldstore items: ${this.state.localForageLength}`}
          </div>
          <div className="debug-info-item">
            {`Notifications: ${Object.keys(notifications).length}`}
          </div>
          <div className="debug-info-item">
            {`Processes: ${Object.keys(processes).length}`}
          </div>
          <div className="debug-info-item">
            {`Enqueue batches: ${enqueue_uris_batches.length}`}
          </div>
          <div className="debug-info-item">
            {`Cached URLs: ${Object.keys(getStorage('cache')).length}`}
          </div>
        </div>

        <div className="debug-info-section">
          <h5>Config</h5>
          <div className="debug-info-item">
            {`Slim mode: ${slim_mode ? 'on' : 'off'}`}
          </div>
          <div className="debug-info-item">
            {`Test mode: ${test_mode ? 'on' : 'off'}`}
          </div>
          <div className="debug-info-item">
            {`Touch: ${isTouchDevice() ? 'on' : 'off'}`}
          </div>
          <div className="debug-info-item">
            {`LocalStorage usage: ${localStorageUsage.used}kb (~${localStorageUsage.percent}%)`}
          </div>
          <div className="debug-info-item">
            {`Selected tracks: ${selected_tracks.length}`}
            <br />
            {
							selected_tracks.map((track_key, index) => (
  <div key={`${track_key}_${index}`}>{track_key}</div>
							))
						}
          </div>
        </div>

        <div className="debug-info-section">
          <h5>Load queue</h5>
          {this.renderLoadQueue()}
        </div>

      </div>
    );
  }
}

const mapStateToProps = (state) => {
  const {
    core: {
      items,
    },
    ui: {
      notifications,
      processes,
      slim_mode,
      test_mode,
      selected_tracks,
      load_queue,
    },
    mopidy: {
      enqueue_uris_batches,
    },
  } = state;

  return {
    items,
    notifications,
    processes,
    slim_mode,
    test_mode,
    selected_tracks,
    enqueue_uris_batches,
    load_queue,
  };
};

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(DebugInfo);

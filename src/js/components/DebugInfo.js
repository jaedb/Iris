
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { get as getStorage } from '../util/storage';
import { isTouchDevice } from '../util/helpers';
import * as uiActions from '../services/ui/actions';
import { indexToArray } from '../util/arrays';

class DebugInfo extends React.Component {
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

  renderLoadQueue() {
    const { ui: { load_queue } } = this.props;
    if (!load_queue) return <div className="debug-info-item mid_grey-text">Nothing loading</div>;

    const queue = indexToArray(load_queue, null, true);

    if (queue.length > 0) {
      return (
        <div className="debug-info-item">
          {queue.map((item, index) => (<div key={`${item}_${index}`}>{item}</div>))}
        </div>
      );
    }
    return <div className="debug-info-item mid_grey-text">Nothing loading</div>;
  }

  render() {
    const localStorageUsage = this.localStorageSize();

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
          <h5>Indexes</h5>
          <div className="debug-info-item">
						Albums:
            {' '}
            {this.props.core.albums ? Object.keys(this.props.core.albums).length : '0'}
          </div>
          <div className="debug-info-item">
						Artists:
            {' '}
            {this.props.core.artists ? Object.keys(this.props.core.artists).length : '0'}
          </div>
          <div className="debug-info-item">
						Playlists:
            {' '}
            {this.props.core.playlists ? Object.keys(this.props.core.playlists).length : '0'}
          </div>
          <div className="debug-info-item">
						Tracks:
            {' '}
            {this.props.core.tracks ? Object.keys(this.props.core.tracks).length : '0'}
          </div>
          <div className="debug-info-item">
						Users:
            {' '}
            {this.props.core.users ? Object.keys(this.props.core.users).length : '0'}
          </div>
          <div className="debug-info-item">
						Notifications:
            {' '}
            {this.props.ui.notifications ? Object.keys(this.props.ui.notifications).length : '0'}
          </div>
          <div className="debug-info-item">
						Processes:
            {' '}
            {this.props.ui.processes ? Object.keys(this.props.ui.processes).length : '0'}
          </div>
          <div className="debug-info-item">
						Enqueue batches:
            {' '}
            {this.props.mopidy.enqueue_uris_batches ? this.props.mopidy.enqueue_uris_batches.length : '0'}
          </div>
          <div className="debug-info-item">
						Cached URLs:
            {' '}
            {Object.keys(getStorage('cache')).length}
          </div>
        </div>

        <div className="debug-info-section">
          <h5>Config</h5>
          <div className="debug-info-item">
						Slim mode:
            {' '}
            {this.props.ui.slim_mode ? 'on' : 'off'}
          </div>
          <div className="debug-info-item">
						Test mode:
            {' '}
            {this.props.ui.test_mode ? 'on' : 'off'}
          </div>
          <div className="debug-info-item">
						Touch:
            {' '}
            {isTouchDevice() ? 'on' : 'off'}
          </div>
          <div className="debug-info-item">
						LocalStorage usage:
            {' '}
            {localStorageUsage.used}
kb (~
            {localStorageUsage.percent}
%)
          </div>
          <div className="debug-info-item">
						Selected tracks:
            {' '}
            {this.props.ui.selected_tracks.length}
            <br />
            {
							this.props.ui.selected_tracks.map((track_key, index) => (
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

const mapStateToProps = (state, ownProps) => ({
  core: state.core,
  ui: state.ui,
  mopidy: state.mopidy,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(DebugInfo);

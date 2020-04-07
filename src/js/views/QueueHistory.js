
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import TrackList from '../components/TrackList';
import Header from '../components/Header';
import Icon from '../components/Icon';
import * as uiActions from '../services/ui/actions';
import * as pusherActions from '../services/pusher/actions';
import * as spotifyActions from '../services/spotify/actions';
import * as mopidyActions from '../services/mopidy/actions';

class QueueHistory extends React.Component {
  componentDidMount() {
    this.props.uiActions.setWindowTitle('Queue history');
    this.loadHistory();
  }

  componentDidUpdate = ({ mopidy_connected: prev_mopidy_connected }) => {
    const { mopidy_connected } = this.props;
    if (!prev_mopidy_connected && mopidy_connected) this.loadHistory();
  }

  loadHistory(props = this.props) {
    if (props.mopidy_connected) {
      this.props.mopidyActions.getQueueHistory();
    }
  }

  render() {
    const options = (
      <a className="button button--no-hover" onClick={(e) => this.props.history.push('/queue')}>
        <Icon name="keyboard_backspace" />
&nbsp;
				Back
      </a>
    );

    const tracks = [];
    for (let i = 0; i < this.props.queue_history.length; i++) {
      let track = { ...this.props.queue_history[i] };

      // We have the track in the index, so merge the track objects
      if (this.props.tracks[track.uri] !== undefined) {
        track = {

          ...track,
          ...this.props.tracks[track.uri],
        };
      }

      tracks.push(track);
    }

    return (
      <div className="view queue-history-view">
        <Header options={options} uiActions={this.props.uiActions}>
          <Icon name="play_arrow" type="material" />
					Playback history
        </Header>
        <section className="content-wrapper">
          <TrackList
            uri="iris:queue-history"
            className="queue-history-track-list"
            track_context="history"
            tracks={tracks}
            show_source_icon
          />
        </section>

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
  mopidy_connected: state.mopidy.connected,
  tracks: (state.core.tracks ? state.core.tracks : {}),
  queue_history: (state.mopidy.queue_history ? state.mopidy.queue_history : []),
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  pusherActions: bindActionCreators(pusherActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(QueueHistory);

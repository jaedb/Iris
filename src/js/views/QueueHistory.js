
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
import { i18n, I18n } from '../locale';

class QueueHistory extends React.Component {
  componentDidMount() {
    const { uiActions: { setWindowTitle } } = this.props;
    setWindowTitle(i18n('queue_history.title'));
    this.loadHistory();
  }

  componentDidUpdate = ({ mopidy_connected: prev_mopidy_connected }) => {
    const { mopidy_connected } = this.props;
    if (!prev_mopidy_connected && mopidy_connected) this.loadHistory();
  }

  loadHistory = (props = this.props) => {
    const {
      mopidyActions: { getQueueHistory },
    } = this.props;

    if (props.mopidy_connected) {
      getQueueHistory();
    }
  }

  onBack = () => {
    const { history: { push } } = this.props;
    push('/queue');
  }

  render = () => {
    const {
      queue_history,
      tracks: tracksProp,
      uiActions,
    } = this.props;

    const options = (
      <a className="button button--no-hover" onClick={this.onBack}>
        <I18n path="actions.back">
          <Icon name="keyboard_backspace" />
          {' '}
        </I18n>
      </a>
    );

    const tracks = queue_history.map((item) => ({
      ...item,
      ...(tracksProp[item.uri] || {}),
    }));

    return (
      <div className="view queue-history-view">
        <Header options={options} uiActions={uiActions}>
          <I18n path="queue_history.title">
            <Icon name="play_arrow" type="material" />
          </I18n>
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

const mapStateToProps = (state) => ({
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


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
import Button from '../components/Button';
import { queueHistorySelector } from '../util/selectors';

class QueueHistory extends React.Component {
  componentDidMount() {
    const { uiActions: { setWindowTitle } } = this.props;
    setWindowTitle(i18n('queue_history.title'));
    this.loadHistory();
  }

  loadHistory = () => {
    const {
      mopidyActions: {
        getQueueHistory,
      },
    } = this.props;

    getQueueHistory();
  }

  onBack = () => {
    const { history: { push } } = this.props;
    push('/queue');
  }

  render = () => {
    const {
      tracks,
      uiActions,
    } = this.props;

    const options = (
      <Button
        onClick={this.onBack}
        noHover
        tracking={{ category: 'QueueHistory', action: 'Back' }}
      >
        <I18n path="actions.back">
          <Icon name="keyboard_backspace" />
          {' '}
        </I18n>
      </Button>
    );

    return (
      <div className="view queue-history-view">
        <Header options={options} uiActions={uiActions}>
          <I18n path="queue_history.title">
            <Icon name="play_arrow" type="material" />
          </I18n>
        </Header>
        {tracks.length > 0 && (
          <section className="content-wrapper">
            <TrackList
              uri="iris:queue-history"
              className="queue-history-track-list"
              track_context="history"
              tracks={tracks}
              show_source_icon
            />
          </section>
        )}
      </div>
    );
  }
}

const mapStateToProps = (state) => ({
  tracks: queueHistorySelector(state),
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  pusherActions: bindActionCreators(pusherActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(QueueHistory);

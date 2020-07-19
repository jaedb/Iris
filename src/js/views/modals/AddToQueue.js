
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Modal from './Modal';
import * as coreActions from '../../services/core/actions';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { I18n, i18n } from '../../locale';

class AddToQueue extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      uris: '',
      next: false,
    };
  }

  componentDidMount() {
    this.props.uiActions.setWindowTitle(i18n('modal.add_to_queue.title'));
  }

  handleSubmit(e) {
    e.preventDefault();
    const uris = this.state.uris.split(',');
    this.props.mopidyActions.enqueueURIs(uris, null, this.state.next);
    window.history.back();
  }

  render() {
    return (
      <Modal className="modal--add-to-queue">
        <h1>
          <I18n path="modal.add_to_queue.title"/>
        </h1>
        <h2 className="mid_grey-text">
          <I18n path="modal.add_to_queue.subtitle" />
        </h2>

        <form onSubmit={(e) => this.handleSubmit(e)}>
          <div className="field text">
            <div className="name">
              <I18n path="modal.add_to_queue.uris" />
            </div>
            <div className="input">
              <input
                type="text"
                onChange={(e) => this.setState({ uris: e.target.value })}
                value={this.state.uris}
              />
            </div>
          </div>

          <div className="field radio white">
            <div className="name">
						  <I18n path="modal.add_to_queue.position.label" />
            </div>
            <div className="input">
              <label>
                <input
                  type="radio"
                  name="next"
                  checked={!this.state.next}
                  onChange={(e) => this.setState({ next: false })}
                />
                <span className="label">
                  <I18n path="modal.add_to_queue.position.end" />
                </span>
              </label>
              <label>
                <input
                  type="radio"
                  name="next"
                  checked={this.state.next}
                  onChange={(e) => this.setState({ next: true })}
                />
                <span className="label">
                  <I18n path="modal.add_to_queue.position.next" />
                </span>
              </label>
            </div>
          </div>

          <div className="actions centered-text">
            <button type="submit" className="button button--primary button--large">
              <I18n path="actions.add" />
            </button>
          </div>
        </form>
      </Modal>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  mopidy_connected: state.mopidy.connected,
  playlist: (state.core.playlists[ownProps.match.params.uri] !== undefined ? state.core.playlists[ownProps.match.params.uri] : null),
  playlists: state.core.playlists,
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(AddToQueue);

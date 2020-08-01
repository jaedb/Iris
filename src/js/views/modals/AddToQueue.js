
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Modal from './Modal';
import * as coreActions from '../../services/core/actions';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { I18n, i18n } from '../../locale';
import { uriType } from '../../util/helpers';
import Icon from '../../components/Icon';
import TextField from '../../components/Fields/TextField';

const UriListItem = ({
  uri,
  tracks,
  albums,
  remove,
}) => {
  const type = uriType(uri);
  let item = null;
  switch (type) {
    case 'track':
      item = tracks[uri];
      break;
    case 'album':
      item = albums[uri];
      break;
    default:
      break;
  }
  return (
    <div className="list__item">
      {item ? item.name : <span className="mid_grey-text">{uri}</span> }
      <span className="mid_grey-text"> ({type})</span>
      <span className="button discrete remove-uri no-hover" onClick={() => remove(uri)}>
        <Icon name="delete" />
        <I18n path="actions.remove" />
      </span>
    </div>
  )
}

class AddToQueue extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      text: '',
      error: '',
      uris: [],
      next: false,
    };
  }

  componentDidMount() {
    const {
      uiActions: {
        setWindowTitle,
      },
    } = this.props;

    setWindowTitle(i18n('modal.add_to_queue.title'));
  }

  onSubmit = (e) => {
    const {
      uris,
      next,
    } = this.state;
    const {
      mopidyActions: {
        enqueueURIs,
      },
    } = this.props;

    e.preventDefault();
    enqueueURIs(uris, null, next);
    window.history.back();
  }

  onChange = (text) => {
    this.setState({ text });
  }

  addUris = () => {
    const {
      text,
    } = this.state;
    const uris = text.split(',');
    this.setState({ uris, text: '' });
  }

  removeUri = (uri) => {
    const {
      uris,
    } = this.state;

    this.setState({ uris: uris.filter((item) => item !== uri) });
  }

  render = () => {
    const {
      tracks,
      albums,
    } = this.props;
    const {
      uris,
      text,
      error,
    } = this.state;

    return (
      <Modal className="modal--add-to-queue">
        <h1>
          <I18n path="modal.add_to_queue.title"/>
        </h1>
        <h2 className="mid_grey-text">
          <I18n path="modal.add_to_queue.subtitle" />
        </h2>

        <form onSubmit={this.onSubmit}>

        <div className="field text">
            <div className="name">
              <I18n path="modal.add_to_queue.items_to_add" />
            </div>
            <div className="input">
              {uris.length ? (
                <div className="list">
                  {uris.map((uri) => (
                    <UriListItem
                      uri={uri}
                      tracks={tracks}
                      albums={albums}
                      remove={this.removeUri}
                      key={uri}
                    />
                  ))}
                </div>
              ) : (
                <span className="text grey-text"><I18n path="modal.add_to_queue.no_items" /></span>
              )}
            </div>
          </div>

          <div className="field text">
            <div className="name">
              <I18n path="fields.uri" />
            </div>
            <div className="input">
              <TextField
                onChange={this.onChange}
                value={text}
              />
              <span className="button discrete add-uri no-hover" onClick={this.addUris}>
                <Icon name="add" />
                <I18n path="actions.add" />
              </span>
              {error && <span className="description error">{error}</span>}
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
  albums: state.core.albums,
  tracks: state.core.tracks,
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(AddToQueue);


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
import LinksSentence from '../../components/LinksSentence';
import Button from '../../components/Button';

const UriListItem = ({
  uri,
  tracks,
  random_tracks,
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
  if (!item) {
    item = random_tracks.find((track) => track.uri === uri);
  }
  return (
    <div className="list__item list__item--no-interaction">
      {item ? item.name : <span className="mid_grey-text">{uri}</span>}
      {item && item.artists && (
        <I18n path="common.by" contentAfter>
          <LinksSentence nolinks items={item.artists} />
        </I18n>
      )}
      <span className="mid_grey-text">{` (${type})`}</span>
      <Button
        type="destructive"
        discrete
        size="tiny"
        className="pull-right"
        onClick={() => remove(uri)}
        tracking={{ category: 'AddToQueue', action: 'RemoveItem' }}
      >
        <Icon name="delete" />
        <I18n path="actions.remove" />
      </Button>
    </div>
  );
};

class AddToQueue extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      text: '',
      error: '',
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
      next,
    } = this.state;
    const {
      mopidyActions: {
        enqueueURIs,
      },
      view: {
        uris = [],
      },
    } = this.props;

    e.preventDefault();
    enqueueURIs(uris, null, next);
    this.reset();
    window.history.back();
  }

  onChange = (text) => {
    this.setState({ text });
  }

  reset = () => {
    const {
      coreActions: {
        viewDataLoaded,
      },
    } = this.props;

    viewDataLoaded({ uris: [], random_tracks: [] });
  }

  addRandom = () => {
    const {
      mopidyActions: {
        view_getRandomTracks,
      },
    } = this.props;

    view_getRandomTracks(20);
  }

  addUris = () => {
    const {
      text,
    } = this.state;
    const {
      view: {
        uris: prevUris = [],
      },
      coreActions: {
        loadItem,
        viewDataLoaded,
      },
    } = this.props;
    const uris = text.split(',');

    const validatedUris = uris.filter((uri) => uriType(uri));

    validatedUris.forEach((uri) => {
      loadItem(uri);
    });

    this.setState({ text: '' });
    viewDataLoaded({ uris: [...prevUris, ...validatedUris] });
  }

  removeUri = (uri) => {
    const {
      view: {
        uris = [],
      },
      coreActions: {
        viewDataLoaded,
      },
    } = this.props;

    viewDataLoaded({ uris: uris.filter((item) => item !== uri) });
  }

  render = () => {
    const {
      tracks,
      albums,
      view: {
        uris = [],
        random_tracks = [],
      },
    } = this.props;
    const {
      text,
      error,
      next,
    } = this.state;

    return (
      <Modal className="modal--add-to-queue">
        <h1>
          <I18n path="modal.add_to_queue.title" />
        </h1>
        <h2 className="mid_grey-text">
          <I18n path="modal.add_to_queue.subtitle" />
        </h2>

        <form onSubmit={this.onSubmit}>

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

          <div className="field text">
            <div className="name">
              <I18n path="fields.items_to_add.label" />
            </div>
            <div className="input">
              {uris.length ? (
                <div className="list">
                  {uris.map((uri, index) => (
                    <UriListItem
                      uri={uri}
                      tracks={tracks}
                      random_tracks={random_tracks}
                      albums={albums}
                      remove={this.removeUri}
                      key={`${uri}_${index}`}
                    />
                  ))}
                </div>
              ) : (
                <span className="text mid_grey-text">
                  <I18n path="fields.items_to_add.placeholder" />
                </span>
              )}
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
                  checked={!next}
                  onChange={() => this.setState({ next: false })}
                />
                <span className="label">
                  <I18n path="modal.add_to_queue.position.end" />
                </span>
              </label>
              <label>
                <input
                  type="radio"
                  name="next"
                  checked={next}
                  onChange={() => this.setState({ next: true })}
                />
                <span className="label">
                  <I18n path="modal.add_to_queue.position.next" />
                </span>
              </label>
            </div>
          </div>

          <div className="actions centered-text">
            <Button
              colour="grey"
              size="large"
              onClick={this.addRandom}
              tracking={{ category: 'AddToQueue', action: 'AddRandom' }}
            >
              <I18n path="modal.add_to_queue.add_random" />
            </Button>
            <Button
              type="primary"
              size="large"
              disabled={!uris.length}
              submit
              tracking={{ category: 'AddToQueue', action: 'Submit' }}
            >
              <I18n path="actions.add" />
            </Button>
          </div>
        </form>
      </Modal>
    );
  }
}

const mapStateToProps = (state) => ({
  albums: state.core.albums,
  tracks: state.core.tracks,
  view: state.core.view ? state.core.view : {},
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(AddToQueue);

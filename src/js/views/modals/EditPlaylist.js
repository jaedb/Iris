
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Modal from './Modal';
import * as coreActions from '../../services/core/actions';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { uriSource } from '../../util/helpers';
import { i18n, I18n } from '../../locale';
import Button from '../../components/Button';
import { decodeUri } from '../../util/format';
import { makeItemSelector, makeLoadingSelector } from '../../util/selectors';

class EditPlaylist extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      loaded: false,
      error: null,
      name: '',
      description: '',
      image: null,
      public: false,
      collaborative: false,
    };
  }

  componentDidMount() {
    const {
      uri,
      playlist,
      coreActions: {
        loadPlaylist,
      },
      uiActions: {
        setWindowTitle,
      },
    } = this.props;

    setWindowTitle(i18n('modal.edit_playlist.title'));

    if (playlist) {
      this.setState({
        loaded: true,
        name: playlist.name,
        description: playlist.description,
        public: (playlist.public === true),
        collaborative: (playlist.collaborative === true),
      });
    } else {
      loadPlaylist(uri);
    }
  }

  componentDidUpdate = ({
    playlist: prevPlaylist,
  }) => {
    const {
      uri,
      playlist,
      mopidyActions: {
        getPlaylist,
      },
    } = this.props;

    if (playlist !== prevPlaylist) {
      getPlaylist(uri);
    }
  }

  static getDerivedStateFromProps({ playlist }, state) {
    if (playlist && !state.loaded) {
      return {
        loaded: true,
        name: playlist.name,
        description: playlist.description,
        public: (playlist.public === true),
        collaborative: (playlist.collaborative === true),
      };
    }
    return null;
  }

  setImage = (e) => {
    const self = this;

    // Create a file-reader to import the selected image as a base64 string
    const file_reader = new FileReader();

    // Once the image is loaded, convert the result
    file_reader.addEventListener('load', (e) => {
      const image_base64 = e.target.result.replace('data:image/jpeg;base64,', '');
      self.setState({ image: image_base64 });
    });

    // This calls the filereader to load the file
    file_reader.readAsDataURL(e.target.files[0]);
  }

  savePlaylist = (e) => {
    const {
      name,
      description,
      public: isPublic,
      collaborative,
      image,
    } = this.state;
    const {
      uri,
      coreActions: {
        savePlaylist,
      },
    } = this.props;

    e.preventDefault();

    if (!name || name == '') {
      this.setState({ error: i18n('modal.edit_playlist.name_required') });
      return false;
    }
    savePlaylist(
      uri,
      name,
      description,
      isPublic,
      collaborative,
      image,
    );
    window.history.back();
    return false;
  }

  renderFields = () => {
    const {
      uri,
    } = this.props;
    const {
      name,
      description,
      public: isPublic,
      collaborative,
      image,
    } = this.state;

    switch (uriSource(uri)) {
      case 'spotify':
        return (
          <div>
            <div className="field text">
              <div className="name">
                <I18n path="modal.edit_playlist.name" />
              </div>
              <div className="input">
                <input
                  type="text"
                  onChange={(e) => this.setState({ name: e.target.value })}
                  value={name}
                />
              </div>
            </div>
            <div className="field text">
              <div className="name">
                <I18n path="modal.edit_playlist.description" />
              </div>
              <div className="input">
                <input
                  type="text"
                  onChange={(e) => this.setState({ description: e.target.value })}
                  value={description}
                />
              </div>
            </div>
            <div className="field file">
              <div className="name">
                <I18n path="modal.edit_playlist.image.label" />
              </div>
              <div className="input">
                <input
                  type="file"
                  placeholder="Leave empty to keep existing image"
                  onChange={(e) => this.setImage(e)}
                />
                <div className="description">
									<I18n path="modal.edit_playlist.image.description" />
                </div>
              </div>
            </div>
            <div className="field checkbox white">
              <div className="name">
								<I18n path="modal.edit_playlist.options.label" />
              </div>
              <div className="input">
                <label>
                  <input
                    type="checkbox"
                    name="playlist_private"
                    checked={isPublic}
                    onChange={() => this.setState({ public: !isPublic })}
                  />
                  <span className="label">
                    <I18n path="modal.edit_playlist.options.public" />
                  </span>
                </label>
                <label>
                  <input
                    type="checkbox"
                    name="collaborative"
                    checked={collaborative}
                    onChange={() => this.setState({ collaborative: !collaborative })}
                  />
                  <span className="label">
                    <I18n path="modal.edit_playlist.options.collaborative" />
                  </span>
                </label>
              </div>
            </div>
          </div>
        );

      default:
        return (
          <div>
            <div className="field text">
              <div className="name">
                <I18n path="modal.edit_playlist.name" />
              </div>
              <div className="input">
                <input
                  type="text"
                  onChange={(e) => this.setState({ name: e.target.value })}
                  value={name}
                />
              </div>
            </div>
          </div>
        );
    }
  }

  render = () => {
    const {
      error,
    } = this.state;

    return (
      <Modal className="modal--edit-playlist">
        <h1>
          <I18n path="modal.edit_playlist.title" />
        </h1>
        {error ? <h3 className="red-text">{error}</h3> : null}
        <form onSubmit={(e) => this.savePlaylist(e)}>

          {this.renderFields()}

          <div className="actions centered-text">
            <Button
              type="primary"
              size="large"
              tracking={{ category: 'EditPlaylist', action: 'Submit' }}
              submit
            >
              <I18n path="actions.save" />
            </Button>
          </div>
        </form>
      </Modal>
    );
  }
}

const mapStateToProps = (state, ownProps) => {
  const uri = decodeUri(ownProps.match.params.uri);
  const itemSelector = makeItemSelector(uri);
  const loadingSelector = makeLoadingSelector([`(.*)${uri}(.*)`, '^((?!contains).)*$', '^((?!tracks).)*$']);

  return {
    uri,
    playlist: itemSelector(state),
    loading: loadingSelector(state),
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(EditPlaylist);

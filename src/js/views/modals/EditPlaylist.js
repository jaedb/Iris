
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
    this.props.uiActions.setWindowTitle(i18n('modal.edit_playlist.title'));

    if (this.props.playlist) {
      this.setState({
        loaded: true,
        name: this.props.playlist.name,
        description: this.props.playlist.description,
        public: (this.props.playlist.public == true),
        collaborative: (this.props.playlist.collaborative == true),
      });
    } else {
      switch (uriSource(this.props.uri)) {
        case 'spotify':
          this.props.spotifyActions.getPlaylist(this.props.uri);
          this.props.spotifyActions.following(this.props.uri);
          break;

        default:
          this.props.mopidyActions.getPlaylist(this.props.uri);
          break;
      }
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

  savePlaylist(e) {
    e.preventDefault();

    if (!this.state.name || this.state.name == '') {
      this.setState({ error: i18n('modal.edit_playlist.name_required') });
      return false;
    }
    this.props.coreActions.savePlaylist(
      this.props.uri,
      this.state.name,
      this.state.description,
      this.state.public,
      this.state.collaborative,
      this.state.image,
    );
    window.history.back();
    return false;
  }

  setImage(e) {
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

  renderFields() {
    switch (uriSource(this.props.uri)) {
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
                  value={this.state.name}
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
                  value={this.state.description}
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
                    checked={this.state.public}
                    onChange={(e) => this.setState({ public: !this.state.public })}
                  />
                  <span className="label">
                    <I18n path="modal.edit_playlist.options.public" />
                  </span>
                </label>
                <label>
                  <input
                    type="checkbox"
                    name="collaborative"
                    checked={this.state.collaborative}
                    onChange={(e) => this.setState({ collaborative: !this.state.collaborative })}
                  />
                  <span className="label">
                    <I18n path="modal.edit_playlist.options.collaborative" />
                  </span>
                </label>
              </div>
            </div>
          </div>
        );
        break;

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
                  value={this.state.name}
                />
              </div>
            </div>
          </div>
        );
    }
  }

  render() {
    return (
      <Modal className="modal--edit-playlist">
        <h1>
          <I18n path="modal.edit_playlist.title" />
        </h1>
        {this.state.error ? <h3 className="red-text">{this.state.error}</h3> : null}
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
  // Decode the URI, and then re-encode selected characters
  // This is needed as Mopidy encodes *some* characters in playlist URIs (but not other characters)
  // We need to retain ":" because this a reserved URI separator
  let uri = decodeURIComponent(ownProps.match.params.uri);
  uri = uri.replace(/\s/g, '%20');	// space
  uri = uri.replace(/\[/g, '%5B');	// [
  uri = uri.replace(/\]/g, '%5D');	// ]
  uri = uri.replace(/\(/g, '%28');	// (
  uri = uri.replace(/\)/g, '%29');	// )
  uri = uri.replace(/\#/g, '%23');	// #

  return {
    uri,
    playlist: (state.core.playlists[uri] !== undefined ? state.core.playlists[uri] : null),
    playlists: state.core.playlists,
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(EditPlaylist);

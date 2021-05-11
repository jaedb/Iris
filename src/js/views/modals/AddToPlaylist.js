import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Loader from '../../components/Loader';
import Modal from './Modal';
import Icon from '../../components/Icon';
import Thumbnail from '../../components/Thumbnail';
import * as coreActions from '../../services/core/actions';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { sourceIcon } from '../../util/helpers';
import { sortItems } from '../../util/arrays';
import { I18n, i18n } from '../../locale';
import { decodeUri } from '../../util/format';
import {
  makeProcessProgressSelector,
  makeProvidersSelector,
  makeLibrarySelector,
} from '../../util/selectors';

const processKeys = [
  'MOPIDY_GET_LIBRARY_PLAYLISTS',
  'SPOTIFY_GET_LIBRARY_PLAYLISTS',
];

class AddToPlaylist extends React.Component {
  componentDidMount = () => {
    const {
      uiActions: {
        setWindowTitle,
      },
    } = this.props;

    this.getLibraries();
    setWindowTitle(i18n('modal.add_to_playlist.title'));
  }

  getLibraries = () => {
    const {
      providers,
      coreActions: {
        loadLibrary,
      },
    } = this.props;

    providers.forEach((provider) => loadLibrary(provider.uri, 'playlists'));
  };

  playlistSelected = (playlist_uri) => {
    const {
      coreActions: {
        addTracksToPlaylist,
      },
      uris,
    } = this.props;
    addTracksToPlaylist(playlist_uri, uris);
    window.history.back();
  }

  renderList = () => {
    const {
      playlists: allPlaylists,
      loading_progress,
    } = this.props;

    if (loading_progress) {
      return <Loader body loading progress={loading_progress} />;
    }

    let playlists = allPlaylists.filter((playlist) => playlist.can_edit);
    playlists = sortItems(playlists, 'name');

    if (playlists.length > 0) {
      return (
        <div className="list small playlists">
          {playlists.map((playlist) => (
            <div
              className="list__item"
              key={playlist.uri}
              onClick={() => this.playlistSelected(playlist.uri)}
            >
              <Thumbnail images={playlist.images} size="small" />
              <h4 className="list__item__name">{ playlist.name }</h4>
              <ul className="list__item__details details">
                <li><Icon type="fontawesome" className="source" name={sourceIcon(playlist.uri)} /></li>
                <li className="mid_grey-text">
                  {`${playlist.tracks_total || 0} tracks`}
                </li>
              </ul>
            </div>
          ))}
        </div>
      );
    }

    return (
      <div className="no-results">
        <I18n path="modal.add_to_playlist.no_playlists" />
      </div>
    );
  }

  render = () => {
    const { uris } = this.props;

    return (
      <Modal className="modal--add-to-playlist">
        <h1><I18n path="modal.add_to_playlist.title" /></h1>
        <h2 className="mid_grey-text">
          <I18n
            path="modal.add_to_playlist.subtitle"
            count={uris.length}
            plural={uris.length > 1 ? 's' : ''}
          />
        </h2>
        {this.renderList()}
      </Modal>
    );
  }
}

const librarySelector = makeLibrarySelector('playlists', false);
const processProgressSelector = makeProcessProgressSelector(processKeys);
const providersSelector = makeProvidersSelector('playlists');
const mapStateToProps = (state, ownProps) => {
  const {
    spotify: {
      me: {
        id: me_id,
      } = {},
      access_token: spotify_available,
    },
  } = state;

  const unencodedUris = ownProps.match.params?.uris;
  const uris = unencodedUris ? decodeUri(unencodedUris).split(',') : [];

  return {
    uris,
    providers: providersSelector(state),
    playlists: librarySelector(state, 'playlists'),
    spotify_available,
    loading_progress: processProgressSelector(state),
    me_id,
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(AddToPlaylist);

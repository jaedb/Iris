
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
import { sourceIcon, decodeMopidyUri } from '../../util/helpers';
import { sortItems } from '../../util/arrays';

class AddToPlaylist extends React.Component {
  componentDidMount = () => {
    const {
      spotify_library_playlists_status,
      mopidy_library_playlists_status,
      mopidy_connected,
      spotify_available,
      spotifyActions,
      mopidyActions,
    } = this.props;

    if ((!spotify_library_playlists_status || spotify_library_playlists_status !== 'finished') && spotify_available) {
      spotifyActions.getLibraryPlaylists();
    }

    if ((!mopidy_library_playlists_status || mopidy_library_playlists_status !== 'finished') && mopidy_connected) {
      mopidyActions.getLibraryPlaylists();
    }
  }

  playlistSelected = (playlist_uri) => {
    const {
      coreActions: {
        addTracksToPlaylist,
      },
      uris,
    } = this.props;
    const encodedUris = uris.map((uri) => decodeMopidyUri(uri));
    addTracksToPlaylist(playlist_uri, encodedUris);
    window.history.back();
  }

  render = () =>{
    const { playlists, uris, spotify_library_playlists_status } = this.props;

    if (!playlists) return <div className="empty">No editable playlists</div>;

    let editablePlaylists = [];
    for (let uri in playlists) {
      if (playlists[uri].can_edit) editablePlaylists.push(playlists[uri]);
    }

    editablePlaylists = sortItems(editablePlaylists, 'name');

    const isLoading = spotify_library_playlists_status === 'running';

    return (
      <Modal className="modal--add-to-playlist">
        <h1>Add to playlist</h1>
        <h2 className="mid_grey-text">
          {`Select playlist to add ${uris.length} track ${uris.length > 1 ? 's' : ''} to`}
        </h2>
        {editablePlaylists.length <= 0 && <div className="no-results">No playlists available</div>}
        <div className="list small playlists">
          {editablePlaylists.map((playlist) => (
            <div
              className="list__item"
              key={playlist.uri}
              onClick={() => this.playlistSelected(playlist.uri)}
            >
              <Thumbnail images={playlist.images} size="small" />
              <h4 className="list__item__name">{ playlist.name }</h4>
              <ul className="list__item__details details">
                <li><Icon type="fontawesome" className="source" name={sourceIcon(playlist.uri)} /></li>
                <li>
                  {playlist.tracks_total && (
                    <span className="mid_grey-text">
                      {`${playlist.tracks_total} tracks`}
                    </span>
                  )}
                </li>
              </ul>
            </div>
          ))}
        </div>
        {isLoading && <Loader body lazy loading />}
      </Modal>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  uris: (ownProps.match.params.uris ? decodeURIComponent(ownProps.match.params.uris).split(',') : []),
  mopidy_connected: state.mopidy.connected,
  mopidy_uri_schemes: state.mopidy.uri_schemes,
  mopidy_library_playlists: state.mopidy.library_playlists,
  mopidy_library_playlists_status: (state.ui.processes.MOPIDY_LIBRARY_PLAYLISTS_PROCESSOR !== undefined ? state.ui.processes.MOPIDY_LIBRARY_PLAYLISTS_PROCESSOR.status : null),
  spotify_available: state.spotify.access_token,
  spotify_library_playlists: state.spotify.library_playlists,
  spotify_library_playlists_status: (state.ui.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR !== undefined ? state.ui.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR.status : null),
  load_queue: state.ui.load_queue,
  me_id: (state.spotify.me ? state.spotify.me.id : false),
  playlists: state.core.playlists,
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(AddToPlaylist);

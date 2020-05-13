
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import ReactGA from 'react-ga';
import ErrorMessage from '../components/ErrorMessage';
import Link from '../components/Link';
import TrackList from '../components/TrackList';
import Thumbnail from '../components/Thumbnail';
import Parallax from '../components/Parallax';
import NiceNumber from '../components/NiceNumber';
import Dater from '../components/Dater';
import LazyLoadListener from '../components/LazyLoadListener';
import FollowButton from '../components/Fields/FollowButton';
import Loader from '../components/Loader';
import ContextMenuTrigger from '../components/ContextMenuTrigger';
import URILink from '../components/URILink';
import Icon from '../components/Icon';
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as spotifyActions from '../services/spotify/actions';
import {
  uriSource,
  getFromUri,
  isLoading,
  sourceIcon,
  decodeMopidyUri,
} from '../util/helpers';
import { collate } from '../util/format';

class Playlist extends React.Component {
  constructor(props) {
    super(props);

    let { uri } = props;

    // Spotify upgraded their playlists URI to remove user component (Sept 2018)
    // We accept the old format, and redirect to the new one
    if (uri.includes('spotify:user:')) {
      uri = uri.replace(/spotify:user:([^:]*?):/i, 'spotify:');
      props.history.push(`/playlist/${encodeURIComponent(uri)}`);
    }
  }

  componentDidMount() {
    this.setWindowTitle();
    this.props.coreActions.loadPlaylist(this.props.uri);
  }

  componentDidUpdate = ({
    uri: prevUri,
    playlist: prevPlaylist,
    mopidy_connected: prev_mopidy_connected,
  }) => {
    const {
      uri,
      playlist,
      mopidy_connected,
      coreActions: {
        loadPlaylist,
      },
      history: {
        push,
      },
    } = this.props;

    if (prevPlaylist && playlist && prevPlaylist.moved_to !== playlist.moved_to) {
      push(`/playlist/${encodeURIComponent(playlist.moved_to)}`);
    }

    if (uri !== prevUri) {
      loadPlaylist(uri);
    } else if (!prev_mopidy_connected && mopidy_connected) {
      if (uriSource(uri) !== 'spotify') {
        loadPlaylist(uri);
      }
    }

    if (!prevPlaylist && playlist) this.setWindowTitle(playlist);
    if (prevUri !== uri && playlist) this.setWindowTitle(playlist);
  }

  setWindowTitle(playlist = this.props.playlist) {
    if (playlist) {
      this.props.uiActions.setWindowTitle(`${playlist.name} (playlist)`);
    } else {
      this.props.uiActions.setWindowTitle('Playlist');
    }
  }

  loadMore() {
    this.props.spotifyActions.getMore(
      this.props.playlist.tracks_more,
      {
        parent_type: 'playlist',
        parent_key: this.props.playlist.uri,
        records_type: 'track',
      },
    );
  }

  handleContextMenu(e) {
    const data = {
      e,
      context: 'playlist',
      items: [this.props.playlist],
      uris: [this.props.uri],
    };
    this.props.uiActions.showContextMenu(data);
  }

  play() {
    this.props.mopidyActions.playPlaylist(this.props.playlist.uri);
  }

  follow() {
    if (this.props.allow_reporting) {
	        ReactGA.event({ category: 'Playlist', action: 'Follow', label: this.props.playlist.uri });
	    }
    this.props.spotifyActions.toggleFollowingPlaylist(this.props.playlist.uri, 'PUT');
  }

  // TODO: Once unfollowing occurs, remove playlist from global playlists list
  unfollow() {
    if (this.props.allow_reporting) {
	        ReactGA.event({ category: 'Playlist', action: 'Unfollow', label: this.props.playlist.uri });
	    }
    this.props.spotifyActions.toggleFollowingPlaylist(this.props.playlist.uri, 'DELETE');
  }

  // TODO: Once deletion occurs, remove playlist from global playlists list
  delete() {
    this.props.mopidyActions.deletePlaylist(this.props.playlist.uri);
  }

  reorderTracks(indexes, index) {
    this.props.coreActions.reorderPlaylistTracks(this.props.playlist.uri, indexes, index, this.props.playlist.snapshot_id);
  }

  removeTracks(tracks_indexes) {
    this.props.coreActions.removeTracksFromPlaylist(this.props.playlist.uri, tracks_indexes);
  }

  inLibrary() {
    const library = `${uriSource(this.props.uri)}_library_playlists`;
    return (this.props[library] && this.props[library].indexOf(this.props.uri) > -1);
  }

  renderActions() {
    switch (uriSource(this.props.uri)) {
      case 'm3u':
        return (
          <div className="actions">
            <button className="button button--primary" onClick={(e) => this.play()}>Play</button>
            <Link className="button button--default" to={`/playlist/${encodeURIComponent(this.props.uri)}/edit`}>Edit</Link>
            <ContextMenuTrigger onTrigger={(e) => this.handleContextMenu(e)} />
          </div>
        );

      case 'spotify':
        if (this.props.playlist.can_edit) {
          return (
            <div className="actions">
              <button className="button button--primary" onClick={(e) => this.play()}>Play</button>
              <Link className="button button--default" to={`/playlist/${encodeURIComponent(this.props.uri)}/edit`}>Edit</Link>
              <ContextMenuTrigger onTrigger={(e) => this.handleContextMenu(e)} />
            </div>
          );
        }
        return (
          <div className="actions">
            <button className="button button--primary" onClick={(e) => this.play()}>Play</button>
            <FollowButton uri={this.props.uri} addText="Add to library" removeText="Remove from library" is_following={this.inLibrary()} />
            <ContextMenuTrigger onTrigger={(e) => this.handleContextMenu(e)} />
          </div>
        );

      default:
        return (
          <div className="actions">
            <button className="button button--primary" onClick={(e) => this.play()}>Play</button>
            <ContextMenuTrigger onTrigger={(e) => this.handleContextMenu(e)} />
          </div>
        );
    }
  }

  render() {
    const playlist_id = getFromUri('playlistid', this.props.uri);

    if (!this.props.playlist) {
      if (isLoading(this.props.load_queue, [`spotify_playlists/${playlist_id}?`])) {
        return <Loader body loading />
      }
      return (
        <ErrorMessage type="not-found" title="Not found">
          <p>
Could not find playlist with URI "
            {encodeURIComponent(this.props.uri)}
"
          </p>
        </ErrorMessage>
      );
    }

    const playlist = collate(this.props.playlist, { tracks: this.props.tracks, users: this.props.users });

    let context = 'playlist';
    if (playlist.can_edit) {
      context = 'editable-playlist';
    }

    if (playlist.tracks_total !== 0 && (!playlist.tracks_uris || (playlist.tracks_uris && !playlist.tracks) || (playlist.tracks_uris.length !== playlist.tracks.length))) {
      var is_loading_tracks = true;
    } else {
      var is_loading_tracks = false;
    }

    return (
      <div className="view playlist-view content-wrapper preserve-3d">

        <div className="thumbnail-wrapper">
          <Thumbnail size="large" glow canZoom images={playlist.images} type="playlist" />
        </div>

        <div className="title">
          <h1>{playlist.name}</h1>
          {playlist.description ? <h2 className="description" dangerouslySetInnerHTML={{ __html: playlist.description }} /> : null }

          <ul className="details">
            {!this.props.slim_mode ? <li className="source"><Icon type="fontawesome" name={sourceIcon(playlist.uri)} /></li> : null }
            {playlist.user_uri ? <li><URILink type="user" uri={playlist.user_uri}>{playlist.user ? playlist.user.name : getFromUri('userid', playlist.user_uri)}</URILink></li> : null }
            <li>
              {playlist.tracks_total ? playlist.tracks_total : '0'}
              {' '}
tracks
            </li>
            {!this.props.slim_mode && playlist.tracks && playlist.tracks_total > 0 ? <li><Dater type="total-time" data={playlist.tracks} /></li> : null }
            {!this.props.slim_mode && playlist.followers !== undefined ? (
              <li>
                <NiceNumber value={playlist.followers} />
                {' '}
followers
              </li>
            ) : null }
            {!this.props.slim_mode && playlist.last_modified_date ? (
              <li>
Edited
                <Dater type="ago" data={playlist.last_modified_date} />
              </li>
            ) : null }
          </ul>
        </div>

        {this.renderActions()}

        <section className="list-wrapper">
          <TrackList
            uri={playlist.uri}
            className="playlist-track-list"
            track_context={context}
            tracks={playlist.tracks}
            removeTracks={(tracks_indexes) => this.removeTracks(tracks_indexes)}
            reorderTracks={(indexes, index) => this.reorderTracks(indexes, index)}
          />
          <LazyLoadListener
            loadKey={playlist.tracks_more}
            showLoader={is_loading_tracks || playlist.tracks_more}
            loadMore={() => this.loadMore()}
          />
        </section>
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => {
  const {
    ui: {
      allow_reporting,
      slim_mode,
      theme,
      load_queue,
    } = {},
    core: {
      users,
      tracks,
      playlists,
    } = {},
    spotify: {
      library_playlists: spotify_library_playlists,
      authorization: spotify_authorized,
      me = {},
    } = {},
    mopidy: {
      connected: mopidy_connected,
      library_playlists: local_library_playlists,
    } = {},
  } = state;

  const uri = decodeMopidyUri(ownProps.match.params.uri);

  return {
    uri,
    allow_reporting,
    slim_mode,
    theme,
    load_queue,
    users,
    tracks,
    playlist: (playlists[uri] !== undefined ? playlists[uri] : false),
    spotify_library_playlists,
    local_library_playlists,
    mopidy_connected,
    spotify_authorized,
    spotify_userid: (me && me.id) || null,
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Playlist);

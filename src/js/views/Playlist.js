import React, { useEffect, useState } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import ErrorMessage from '../components/ErrorMessage';
import Button from '../components/Button';
import TrackList from '../components/TrackList';
import Thumbnail from '../components/Thumbnail';
import PinButton from '../components/Fields/PinButton';
import { nice_number } from '../components/NiceNumber';
import { Dater, dater } from '../components/Dater';
import FollowButton from '../components/Fields/FollowButton';
import Loader from '../components/Loader';
import ContextMenuTrigger from '../components/ContextMenuTrigger';
import URILink from '../components/URILink';
import { SourceIcon } from '../components/Icon';
import DropdownField from '../components/Fields/DropdownField';
import FilterField from '../components/Fields/FilterField';
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as spotifyActions from '../services/spotify/actions';
import { uriSource } from '../util/helpers';
import { i18n, I18n } from '../locale';
import { makeItemSelector, makeLoadingSelector, getSortSelector } from '../util/selectors';
import { sortItems, applyFilter } from '../util/arrays';
import { decodeUri, encodeUri } from '../util/format';

const SORT_KEY = 'playlist_tracks';

const Actions = ({
  encodedUri,
  playlist: {
    uri,
    can_edit,
    name,
    in_library,
  },
  onPlay,
  handleContextMenu,
}) => {
  switch (uriSource(uri)) {
    case 'm3u':
      return (
        <div className="actions">
          <Button
            type="primary"
            onClick={onPlay}
            tracking={{ category: 'Playlist', action: 'Play' }}
          >
            <I18n path="actions.play" />
          </Button>
          <Button
            to={`/playlist/${encodedUri}/edit`}
            tracking={{ category: 'Playlist', action: 'Edit' }}
          >
            <I18n path="actions.edit" />
          </Button>
          <PinButton item={{ uri, name }} />
          <ContextMenuTrigger onTrigger={handleContextMenu} />
        </div>
      );

    case 'spotify':
      if (can_edit) {
        return (
          <div className="actions">
            <Button
              type="primary"
              onClick={onPlay}
              tracking={{ category: 'Playlist', action: 'Play' }}
            >
              <I18n path="actions.play" />
            </Button>
            <Button
              to={`/playlist/${encodedUri}/edit`}
              tracking={{ category: 'Playlist', action: 'Edit' }}
            >
              <I18n path="actions.edit" />
            </Button>
            <PinButton item={{ uri, name }} />
            <ContextMenuTrigger onTrigger={handleContextMenu} />
          </div>
        );
      }
      return (
        <div className="actions">
          <Button
            type="primary"
            onClick={onPlay}
            tracking={{ category: 'Playlist', action: 'Play' }}
          >
            <I18n path="actions.play" />
          </Button>
          <FollowButton
            uri={uri}
            is_following={in_library}
          />
          <PinButton item={{ uri, name }} />
          <ContextMenuTrigger onTrigger={handleContextMenu} />
        </div>
      );

    default:
      return (
        <div className="actions">
          <Button
            type="primary"
            onClick={onPlay}
            tracking={{ category: 'Playlist', action: 'Play' }}
          >
            <I18n path="actions.play" />
          </Button>
          <PinButton item={{ uri, name }} />
          <ContextMenuTrigger onTrigger={handleContextMenu} />
        </div>
      );
  }
}

const Playlist = ({
  loading,
  loading_tracks,
  slim_mode,
  history,
  uri,
  encodedUri,
  name,
  playlist: playlistProp,
  sortField,
  sortReverse,
  coreActions: {
    loadPlaylist,
    reorderPlaylistTracks,
    removeTracksFromPlaylist,
  },
  uiActions: {
    setSort,
    setWindowTitle,
    showContextMenu,
    hideContextMenu,
    createNotification,
  },
  mopidyActions: {
    playPlaylist,
  },
}) => {
  const [playlist, setPlaylist] = useState({});
  const [filter, setFilter] = useState('');

  useEffect(
    () => {
      if (uri) loadPlaylist(uri, { full: true, name });
    },
    [uri],
  );

  useEffect(() => {
    if (playlistProp && playlist && playlistProp.moved_to !== playlist.moved_to) {
      history.push(`/playlist/${encodeUri(playlistProp.moved_to)}`);
    }
  }, [playlistProp])

  useEffect(() => {
    if (playlistProp) {
      setWindowTitle(i18n('playlist.title_window', { name: playlistProp.name }));
    } else {
      setWindowTitle(i18n('playlist.title'));
    }
    setPlaylist(playlistProp);
  }, [playlistProp]);

  const handleContextMenu = (e) => showContextMenu({
    e,
    context: 'playlist',
    items: [{ name, ...playlist }],
    uris: [uri],
  });

  const onChangeSort = (field) => {
    let reverse = false;
    if (field !== null && sortField === field) {
      reverse = !sortReverse;
    }

    setSort(SORT_KEY, field, reverse);
    hideContextMenu();
  };

  const onPlay = () => playPlaylist(uri);

  const reorderTracks = (indexes, index) => {
    const { snapshot_id, tracks } = playlist;

    if (sortField !== 'sort_id' || filter !== '') {
      createNotification({
        content: i18n('errors.cannot_reorder.title'),
        description: i18n('errors.cannot_reorder.description'),
        level: 'error',
      });
      return;
    }

    if (sortReverse) {
      const count = tracks.length - 1;
      index = count - index + 1;
      indexes = indexes.map((index) => count - index);
    }

    reorderPlaylistTracks(uri, indexes, index, snapshot_id);
  }

  const removeTracks = (tracks_indexes) => {
    removeTracksFromPlaylist(uri, tracks_indexes);
  }

  if (loading) {
    return <Loader body loading />;
  }
  if (!playlist) {
    return (
      <ErrorMessage type="not-found" title="Not found">
        <p>
          <I18n path="errors.uri_not_found" uri={uri} />
        </p>
      </ErrorMessage>
    );
  }

  let context = 'playlist';
  if (playlist.can_edit) context = 'editable-playlist';

  let tracks = playlist?.tracks || [];
  if (sortField && tracks) tracks = sortItems(tracks, sortField, sortReverse);
  if (filter && filter !== '') tracks = applyFilter('name', filter, tracks);

  const sort_options = [
    {
      value: 'sort_id',
      label: i18n('playlist.tracks.sort.sort_id'),
    },
    {
      value: 'name',
      label: i18n('playlist.tracks.sort.name'),
    },
    {
      value: 'artist',
      label: i18n('playlist.tracks.sort.artist'),
    },
    {
      value: 'album',
      label: i18n('playlist.tracks.sort.album'),
    },
  ];

  return (
    <div className="view playlist-view content-wrapper preserve-3d">

      <div className="thumbnail-wrapper">
        <Thumbnail size="large" glow canZoom images={playlist.images} type="playlist" />
      </div>

      <div className="title">
        <h1>{playlist.name}</h1>
        {playlist.description && (
          <h2
            className="description"
            dangerouslySetInnerHTML={{ __html: playlist.description }}
          />
        )}

        <ul className="details details--one-line">
          {!slim_mode && (
            <li className="source">
              <SourceIcon uri={playlist.uri} />
            </li>
          )}
          {playlist.user && (
            <li>
              <URILink
                type="user"
                uri={playlist.user.uri}
              >
                {playlist.user.name}
              </URILink>
            </li>
          )}
          <li>
            <I18n path="specs.tracks" count={playlist.tracks ? playlist.tracks.length : 0} />
          </li>
          {!slim_mode && playlist.tracks && playlist.tracks_total > 0 && (
            <li><Dater type="total-time" data={playlist.tracks} /></li>
          )}
          {!slim_mode && playlist.followers !== undefined && (
            <li>
              <I18n path="specs.followers" count={nice_number(playlist.followers)} />
            </li>
          )}
          {!slim_mode && playlist.last_modified_date && (
            <li>
              <I18n path="specs.edited" date={dater('ago', playlist.last_modified_date)} />
            </li>
          )}
        </ul>
      </div>

      <Actions
        encodedUri={encodedUri}
        playlist={playlist}
        onPlay={onPlay}
        handleContextMenu={handleContextMenu}
      />

      <h4 className="no-bottom-margin">
        <I18n path="playlist.tracks.title" />
        {loading_tracks && <Loader loading mini />}
        <div className="actions-wrapper">
          <FilterField
            initialValue={filter}
            handleChange={setFilter}
            onSubmit={hideContextMenu}
          />
          <DropdownField
            icon="swap_vert"
            name="Sort"
            value={sortField}
            valueAsLabel
            options={sort_options}
            selected_icon={sortField ? (sortReverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
            handleChange={onChangeSort}
          />
        </div>
      </h4>

      <section className="list-wrapper no-top-padding">
        <TrackList
          uri={playlist.uri}
          className="playlist-track-list"
          track_context={context}
          tracks={tracks}
          removeTracks={removeTracks}
          reorderTracks={reorderTracks}
        />
      </section>
    </div>
  );
}

const mapStateToProps = (state, ownProps) => {
  const {
    ui: {
      allow_reporting,
      slim_mode,
      theme,
    } = {},
    spotify: {
      library_playlists: spotify_library_playlists,
      authorization: spotify_authorized,
      me = {},
    } = {},
    mopidy: {
      library_playlists: local_library_playlists,
    } = {},
  } = state;

  const uri = decodeUri(ownProps.match.params.uri);
  const itemSelector = makeItemSelector(uri);
  const loadingSelector = makeLoadingSelector([`(.*)${uri}(.*)`, '^((?!contains).)*$', '^((?!tracks).)*$', '^((?!followers).)*$']);
  const loadingTracksSelector = makeLoadingSelector([`(.*)${uri}(.*)tracks(.*)`]);
  const [sortField, sortReverse] = getSortSelector(state, SORT_KEY);

  return {
    uri,
    encodedUri: ownProps.match.params.uri,
    name: ownProps.match.params.name,
    allow_reporting,
    slim_mode,
    theme,
    loading: loadingSelector(state),
    loading_tracks: loadingTracksSelector(state),
    playlist: itemSelector(state),
    spotify_library_playlists,
    local_library_playlists,
    spotify_authorized,
    spotify_userid: (me && me.id) || null,
    sortField,
    sortReverse,
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Playlist);

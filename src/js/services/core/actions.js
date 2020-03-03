
import { createRange } from '../../util/arrays';
import { uriSource } from '../../util/helpers';

const spotifyActions = require('../../services/spotify/actions');
const mopidyActions = require('../../services/mopidy/actions');

export function getBroadcasts() {
  return (dispatch, getState) => {
    const config = {
      method: 'GET',
      timeout: 15000,
    };

    // Fetch the "iris_broadcasts.json" file from Gist (or "_test" for test mode)
    if (getState().ui.test_mode) {
        	config.url = 'https://gist.githubusercontent.com/jaedb/cb3a5ee6909632abb2e0fe66d4c8c311/raw';
    } else {
        	config.url = 'https://gist.githubusercontent.com/jaedb/b677dccf80daf3ccb2ef12e96e495677/raw';
    }

    $.ajax(config).then(
      (response) => {
        dispatch({
          type: 'BROADCASTS_LOADED',
          broadcasts: JSON.parse(response),
        });
      },
      (xhr, status, error) => {
        dispatch(
          handleException(
            'Could not fetch broadcasts from GitHub',
            {
              config,
              xhr,
              status,
              error,
            },
            null,
            false,
          ),
        );
      },
    );
  };
}

export function startSearch(search_type, query, only_mopidy = false) {
  return {
    type: 'SEARCH_STARTED',
    search_type,
    query,
    only_mopidy,
  };
}

export function handleException(message, data = {}, description = null, show_notification = true) {
  if (!message && data.message) {
    message = data.message;
  } else if (!message && data.error.message) {
    message = data.error.message;
  }
  if (!description && data.description) {
    description = data.description;
  } else if (!description && data.error && data.error.description) {
    description = data.error.description;
  }
  return {
    type: 'HANDLE_EXCEPTION',
    message,
    description,
    data,
    show_notification,
  };
}

export function debugResponse(response) {
  return {
    type: 'DEBUG',
    response,
  };
}

export function set(data) {
  return {
    type: 'CORE_SET',
    data,
  };
}

export function clearCurrentTrack() {
  return {
    type: 'CLEAR_CURRENT_TRACK',
  };
}

export function cachebustHttpStream() {
  return {
    type: 'CACHEBUST_HTTP_STREAM',
  };
}

/**
 * Record getters
 *
 * Calling this through the common core enables us to detect whether we've already
 * got the record in the state or persistent storage. Failing that, we pass off to the
 * relevant service to load the record - all from one neat package.
 * */

export function loadTrack(uri, force_reload = false) {
  return {
    type: 'LOAD_TRACK',
    uri,
    force_reload,
  };
}

export function loadAlbum(uri, force_reload = false) {
  return {
    type: 'LOAD_ALBUM',
    uri,
    force_reload,
  };
}

export function loadArtist(uri, force_reload = false) {
  return {
    type: 'LOAD_ARTIST',
    uri,
    force_reload,
  };
}

export function loadPlaylist(uri, force_reload = false) {
  return {
    type: 'LOAD_PLAYLIST',
    uri,
    force_reload,
  };
}

export function loadUser(uri, force_reload = false) {
  return {
    type: 'LOAD_USER',
    uri,
    force_reload,
  };
}

export function loadUserPlaylists(uri, force_reload = false) {
  return {
    type: 'LOAD_USER_PLAYLISTS',
    uri,
    force_reload,
  };
}


/**
 * Record loaders
 *
 * We've got a loaded record, now we just need to plug it in to our state and stores.
 * */

export function trackLoaded(track) {
  return tracksLoaded([track]);
}
export function tracksLoaded(tracks) {
  return {
    type: 'TRACKS_LOADED',
    tracks,
  };
}

export function artistLoaded(artist) {
  return artistsLoaded([artist]);
}
export function artistsLoaded(artists) {
  return {
    type: 'ARTISTS_LOADED',
    artists,
  };
}

export function albumLoaded(album) {
  return albumsLoaded([album]);
}
export function albumsLoaded(albums) {
  return {
    type: 'ALBUMS_LOADED',
    albums,
  };
}

export function playlistLoaded(playlist) {
  return playlistsLoaded([playlist]);
}
export function playlistsLoaded(playlists) {
  return {
    type: 'PLAYLISTS_LOADED',
    playlists,
  };
}

export function userLoaded(user) {
  return usersLoaded([user]);
}
export function usersLoaded(users) {
  return {
    type: 'USERS_LOADED',
    users,
  };
}
export function userPlaylistsLoaded(uri, playlists, more = null, total = null) {
  return {
    type: 'USER_PLAYLISTS_LOADED',
    uri,
    playlists,
    more,
    total,
  };
}

export function loadedMore(parent_type, parent_key, records_type, records_data, extra_data = {}) {
  return {
    type: 'LOADED_MORE',
    parent_type,
    parent_key,
    records_type,
    records_data,
    extra_data,
  };
}

export function removeFromIndex(index_name, key, new_key = null) {
  return {
    type: 'REMOVE_FROM_INDEX',
    index_name,
    key,
    new_key,
  };
}


/**
 * Playlist manipulation
 * */

export function reorderPlaylistTracks(uri, indexes, insert_before, snapshot_id = false) {
  const range = createRange(indexes);
  switch (uriSource(uri)) {
    case 'spotify':
      return {
        type: 'SPOTIFY_REORDER_PLAYLIST_TRACKS',
        key: uri,
        range_start: range.start,
        range_length: range.length,
        insert_before,
        snapshot_id,
      };

    case 'm3u':
      return {
        type: 'MOPIDY_REORDER_PLAYLIST_TRACKS',
        key: uri,
        range_start: range.start,
        range_length: range.length,
        insert_before,
      };
  }
}

export function savePlaylist(uri, name, description = '', is_public = false, is_collaborative = false, image = null) {
  switch (uriSource(uri)) {
    case 'spotify':
      return {
        type: 'SPOTIFY_SAVE_PLAYLIST',
        key: uri,
        name,
        description: (description == '' ? null : description),
        image,
        is_public,
        is_collaborative,
      };

    case 'm3u':
      return {
        type: 'MOPIDY_SAVE_PLAYLIST',
        key: uri,
        name,
      };
  }
  return false;
}

export function createPlaylist(scheme, name, description = '', is_public = false, is_collaborative = false) {
  switch (scheme) {
    case 'spotify':
      if (description == '') {
        description = null;
      }
      return spotifyActions.createPlaylist(name, description, is_public, is_collaborative);

    default:
      return mopidyActions.createPlaylist(name, scheme);
  }
  return false;
}

export function deletePlaylist(uri) {
  switch (uriSource(uri)) {
    case 'spotify':
      return spotifyActions.following(uri, 'DELETE');

    default:
      return mopidyActions.deletePlaylist(uri);
  }
  return false;
}

export function removeTracksFromPlaylist(uri, tracks_indexes) {
  switch (uriSource(uri)) {
    case 'spotify':
      return {
        type: 'SPOTIFY_REMOVE_PLAYLIST_TRACKS',
        key: uri,
        tracks_indexes,
      };

    case 'm3u':
      return {
        type: 'MOPIDY_REMOVE_PLAYLIST_TRACKS',
        key: uri,
        tracks_indexes,
      };
  }
}

export function addTracksToPlaylist(uri, tracks_uris) {
  switch (uriSource(uri)) {
    case 'spotify':
      return {
        type: 'SPOTIFY_ADD_PLAYLIST_TRACKS',
        key: uri,
        tracks_uris,
      };

    case 'm3u':
      return {
        type: 'MOPIDY_ADD_PLAYLIST_TRACKS',
        key: uri,
        tracks_uris,
      };
  }
}


/**
 * Asset libraries
 * */

export function getLibraryPlaylists() {
  return {
    type: 'GET_LIBRARY_PLAYLISTS',
  };
}

export function getLibraryAlbums() {
  return {
    type: 'GET_LIBRARY_ALBUMS',
  };
}

export function getLibraryArtists() {
  return {
    type: 'GET_LIBRARY_ARTISTS',
  };
}

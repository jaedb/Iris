
import { createRange, removeDuplicates } from '../../util/arrays';
import { uriSource } from '../../util/helpers';

const spotifyActions = require('../../services/spotify/actions');
const mopidyActions = require('../../services/mopidy/actions');

export function handleException(message = '', data = {}, description = null, show_notification = true) {
  console.debug({ message, data })
  if (!message) {
    if (data.message) {
      message = data.message;
    } else if (data.error.message) {
      message = data.error.message;
    }
  }
  if (!description) {
    if (data.description) {
      description = data.description;
    } else if (data.error && data.error.message) {
      description = data.error.message;
    } else if (data.error && data.error.description) {
      description = data.error.description;
    }
  }

  // Explicitly convert values to strings
  if (message && typeof message !== 'string') message = String(message);
  if (description && typeof description !== 'string') description = String(description);

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

export function resetState(stateKeysToReset) {
  return {
    type: 'RESET_STATE',
    stateKeysToReset,
  };
}

export function clearCurrentTrack() {
  return {
    type: 'CLEAR_CURRENT_TRACK',
  };
}

export function clearStorage() {
  return {
    type: 'CLEAR_STORAGE',
  };
}

export function restoreItemsFromColdStore(items) {
  return {
    type: 'RESTORE_ITEMS_FROM_COLD_STORE',
    items,
  };
}

export function restoreLibraryFromColdStore(library) {
  return {
    type: 'RESTORE_LIBRARY_FROM_COLD_STORE',
    library,
  };
}

export function updateColdStore(items) {
  return {
    type: 'UPDATE_COLD_STORE',
    items,
  };
}

/**
 * Search results
 */

export function startSearch(query) {
  return {
    type: 'START_SEARCH',
    query,
  };
}

export function searchResultsLoaded(query, resultType, results) {
  return {
    type: 'SEARCH_RESULTS_LOADED',
    query,
    resultType,
    results,
  };
}


/**
 * Record getters
 *
 * Calling this through the common core enables us to detect whether we've already
 * got the record in the state or persistent storage. Failing that, we pass off to the
 * relevant service to load the record - all from one neat package.
 * */

export function loadUris(uris = [], options = {}) {
  return {
    type: 'LOAD_URIS',
    uris,
    options,
  };
}

export function loadUri(uri, options = {}) {
  return loadUris([uri], options);
}

export function loadItems(itemType, uris = [], options = {}) {
  return {
    type: 'LOAD_ITEMS',
    uris,
    options,
    itemType,
  };
}

export function loadItem(uri, options = {}) {
  return loadItems([uri], options);
}

export function loadTrack(uri, options = {}) {
  return {
    type: 'LOAD_TRACK',
    uri,
    options,
  };
}

export function loadAlbum(uri, options = {}) {
  return {
    type: 'LOAD_ALBUM',
    uri,
    options,
  };
}

export function loadArtist(uri, options = {}) {
  return {
    type: 'LOAD_ARTIST',
    uri,
    options,
  };
}

export function loadPlaylist(uri, options = {}) {
  return {
    type: 'LOAD_PLAYLIST',
    uri,
    options,
  };
}

export function loadUser(uri, options = {}) {
  return {
    type: 'LOAD_USER',
    uri,
    options,
  };
}

export function loadUserPlaylists(uri, options = {}) {
  return {
    type: 'LOAD_USER_PLAYLISTS',
    uri,
    options,
  };
}

export function loadLibrary(uri, options = {}) {
  return {
    type: 'LOAD_LIBRARY',
    uri,
    options,
  };
}

export function addToLibrary(uri, item) {
  return {
    type: 'ADD_TO_LIBRARY',
    uri,
    item,
  };
}

export function removeFromLibrary(uri, itemUri) {
  return {
    type: 'REMOVE_FROM_LIBRARY',
    uri,
    itemUri,
  };
}


/**
 * Record loaders
 *
 * We've got a loaded record, now we just need to plug it in to our state and stores.
 * */

export function libraryLoaded(library) {
  return {
    type: 'LIBRARY_LOADED',
    library,
  };
}
export function unloadLibrary(uri) {
  return {
    type: 'UNLOAD_LIBRARY',
    uri,
  };
}
export function itemsLoaded(items) {
  return {
    type: 'ITEMS_LOADED',
    items,
  };
}
export function itemLoaded(item) {
  return itemsLoaded([item]);
}

export function tracksLoaded(tracks) {
  return {
    type: 'TRACKS_LOADED',
    tracks,
  };
}
export function trackLoaded(track) {
  return tracksLoaded([track]);
}

export function artistsLoaded(artists) {
  return {
    type: 'ARTISTS_LOADED',
    artists,
  };
}

export function albumsLoaded(albums) {
  return {
    type: 'ALBUMS_LOADED',
    albums,
  };
}
export function albumLoaded(album) {
  return albumsLoaded([album]);
}

export function playlistsLoaded(playlists) {
  return {
    type: 'PLAYLISTS_LOADED',
    playlists,
  };
}
export function playlistLoaded(playlist) {
  return playlistsLoaded([playlist]);
}

export function usersLoaded(users) {
  return {
    type: 'USERS_LOADED',
    users,
  };
}
export function userLoaded(user) {
  return usersLoaded([user]);
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

export function removeItem(key, new_key = null) {
  return {
    type: 'REMOVE_ITEM',
    key,
    new_key,
  };
}

export function streamTitleLoaded(stream_title) {
  return {
    type: 'STREAM_TITLE_LOADED',
    stream_title,
  };
}

export function streamTitleChanged(stream_title) {
  return {
    type: 'STREAM_TITLE_CHANGED',
    stream_title,
  };
}

export function viewDataLoaded(data) {
  return {
    type: 'VIEW_DATA_LOADED',
    data,
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
    case 'gmusic':
      return {
        type: 'MOPIDY_REORDER_PLAYLIST_TRACKS',
        key: uri,
        range_start: range.start,
        range_length: range.length,
        insert_before,
      };

    default:
      return {
        type: 'UNSUPPORTED_ACTION',
        name: 'reorderPlaylistTracks',
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
    case 'gmusic':
      return {
        type: 'MOPIDY_SAVE_PLAYLIST',
        key: uri,
        name,
      };

    default:
      return {
        type: 'UNSUPPORTED_ACTION',
        name: 'savePlaylist',
      };
  }
}

export function createPlaylist(scheme, name, description = '', is_public = false, is_collaborative = false) {
  switch (scheme) {
    case 'spotify':
      if (description === '') {
        description = null;
      }
      return spotifyActions.createPlaylist(name, description, is_public, is_collaborative);

    default:
      return mopidyActions.createPlaylist(name, scheme);
  }
}

export function deletePlaylist(uri) {
  switch (uriSource(uri)) {
    case 'spotify':
      return spotifyActions.following(uri, 'DELETE');

    default:
      return mopidyActions.deletePlaylist(uri);
  }
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
    case 'gmusic':
      return {
        type: 'MOPIDY_REMOVE_PLAYLIST_TRACKS',
        key: uri,
        tracks_indexes,
      };

    default:
      return {
        type: 'UNSUPPORTED_ACTION',
        name: 'removeTracksFromPlaylist',
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
    case 'gmusic':
      return {
        type: 'MOPIDY_ADD_PLAYLIST_TRACKS',
        key: uri,
        tracks_uris,
      };

    default:
      return {
        type: 'UNSUPPORTED_ACTION',
        name: 'addTracksToPlaylist',
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

export function addPinned(item) {
  return {
    type: 'ADD_PINNED',
    item,
  };
}

export function removePinned(uri) {
  return {
    type: 'REMOVE_PINNED',
    uri,
  };
}

export function updatePinned(pinned) {
  return {
    type: 'UPDATE_PINNED',
    pinned: removeDuplicates(pinned),
  };
}

export function updatePinnedUri(oldUri, newUri) {
  return {
    type: 'UPDATE_PINNED_URI',
    oldUri,
    newUri,
  };
}

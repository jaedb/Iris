import { createSelector } from 'reselect';
import { indexToArray } from './arrays';
import { isLoading } from './helpers';
import { i18n } from '../locale';

const getItem = (state, uri) => state.core.items[uri];
const getItems = (state) => state.core.items;
const getLoadQueue = (state) => state.ui.load_queue;
const getProcesses = (state) => state.ui.processes;
const getLibrary = (state, uri) => state.core.libraries[uri];
const getLibraries = (state) => state.core.libraries;
const getSearchResults = (state) => state.core.search_results;
const getGridGlowEnabled = (state) => state.ui.grid_glow_enabled;
const getDragger = (state) => state.ui.dragger;

const makeItemSelector = (uri) => createSelector(
  [getItems],
  (items) => {
    if (Array.isArray(uri)) {
      return indexToArray(items, uri);
    }
    return items[uri];
  },
);
const makeArtistSelector = (uri) => createSelector(
  [getItems],
  (items) => {
    const artist = items[uri];
    const albums = artist?.albums_uris ? indexToArray(items, artist.albums_uris) : [];
    return {
      ...artist,
      albums,
    };
  },
);
const makeLoadingSelector = (keys) => createSelector(
  [getLoadQueue],
  (loadQueue) => isLoading(loadQueue, keys),
);

const getQueueHistory = (state) => state.mopidy.queue_history;
const queueHistorySelector = createSelector(
  [getItems, getQueueHistory],
  (items, queueHistory) => {
    if (!queueHistory || !queueHistory.length) return [];

    return queueHistory.map((item) => ({
      ...item,
      ...(items[item.uri] || {}),
    }));
  },
);

const getLibrarySource = (state, name) => state.ui[`library_${name}_source`] || 'all';
const makeLibrarySelector = (name, filtered = true) => createSelector(
  [getLibraries, getItems, getLibrarySource],
  (libraries, items, source) => {
    const selectedLibraries = !filtered || source === 'all'
      ? indexToArray(libraries).filter((l) => l.type === name)
      : indexToArray(libraries, [source]);

    const itemUris = selectedLibraries.reduce(
      (acc, library) => [...acc, ...library.items_uris],
      [],
    );
    return indexToArray(items, itemUris);
  },
);

const makeSearchResultsSelector = (term, type) => createSelector(
  [getSearchResults],
  (searchResults) => {
    if (!searchResults || searchResults.query.term !== term) return [];
    return searchResults[type] || [];
  },
);

const makeProcessProgressSelector = (keys) => createSelector(
  [getProcesses],
  (processes) => {
    const selectedProcesses = keys
      .map((key) => processes[key] || {})
      .filter((i) => i.status === 'running');

    if (!selectedProcesses.length) return undefined;

    let total = 0;
    let remaining = 0;
    selectedProcesses.forEach((process) => {
      if (process.total) total += process.total;
      if (process.remaining) remaining += process.remaining;
    });
    return {
      total,
      remaining,
      percent: total && remaining ? ((total - remaining) / total).toFixed(4) : 0,
    };
  },
);

const providers = {
  playlists: [
    {
      scheme: 'm3u:',
      uri: 'm3u:playlists',
      title: i18n('services.mopidy.local'),
    },
    {
      scheme: 'spotify:',
      uri: 'spotify:library:playlists',
      title: i18n('services.spotify.title'),
    },
    {
      scheme: 'tidal:',
      uri: 'tidal:my_playlists',
      title: i18n('services.tidal.title'),
    },
  ],
  albums: [
    {
      scheme: 'local:',
      uri: 'local:directory?type=album',
      title: i18n('services.mopidy.local'),
    },
    {
      scheme: 'gmusic:',
      uri: 'gmusic:album',
      title: i18n('services.google.title'),
    },
    {
      scheme: 'spotify:',
      uri: 'spotify:library:albums',
      title: i18n('services.spotify.title'),
    },
    {
      scheme: 'tidal:',
      uri: 'tidal:my_albums',
      title: i18n('services.tidal.title'),
    },
    {
      scheme: 'ytmusic:',
      uri: 'ytmusic:album',
      title: i18n('services.youtube.title'),
    },
  ],
  artists: [
    {
      scheme: 'local:',
      uri: 'local:directory?type=artist&role=albumartist',
      title: i18n('services.mopidy.local'),
    },
    {
      scheme: 'gmusic:',
      uri: 'gmusic:artist',
      title: i18n('services.google.title'),
    },
    {
      scheme: 'spotify:',
      uri: 'spotify:library:artists',
      title: i18n('services.spotify.title'),
    },
    {
      scheme: 'tidal:',
      uri: 'tidal:my_artists',
      title: i18n('services.tidal.title'),
    },
    {
      scheme: 'ytmusic:',
      uri: 'ytmusic:artist',
      title: i18n('services.youtube.title'),
    },
  ],
  tracks: [
    {
      scheme: 'local:',
      uri: 'local:directory?type=track',
      title: i18n('services.mopidy.local'),
    },
    {
      scheme: 'spotify:',
      uri: 'spotify:library:tracks',
      title: i18n('services.spotify.title'),
    },
  ],
};
const getProvider = (type, scheme) => providers[type]?.find((p) => p.scheme === scheme);
const getUriSchemes = (state) => state.mopidy.uri_schemes || [];
const makeProvidersSelector = (context) => createSelector(
  [getUriSchemes],
  (schemes) => {
    if (!providers[context]) return [];
    return providers[context].filter((p) => schemes.indexOf(p.scheme) > -1);
  },
);

export {
  getItem,
  getLibrary,
  getGridGlowEnabled,
  getLibrarySource,
  getDragger,
  makeItemSelector,
  makeArtistSelector,
  makeLibrarySelector,
  makeLoadingSelector,
  makeSearchResultsSelector,
  makeProcessProgressSelector,
  queueHistorySelector,
  makeProvidersSelector,
  getProvider,
};

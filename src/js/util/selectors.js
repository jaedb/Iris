import { createSelector } from 'reselect';
import { indexToArray } from './arrays';
import { isLoading } from './helpers';

const getItems = (state) => state.core.items;

const makeItemSelector = (uriOrUris) => createSelector(
  [getItems],
  (items) => {
    if (Array.isArray(uriOrUris)) {
      return indexToArray(items, uriOrUris);
    }
    return items[uriOrUris];
  },
);

const getLoadQueue = (state) => state.ui.load_queue;
const makeLoadingSelector = (keys) => createSelector(
  [getLoadQueue],
  (items) => isLoading(items, keys),
);

const getQueueHistory = (state) => state.mopidy.queue_history;
const queueHistorySelector = createSelector(
  [getItems, getQueueHistory],
  (items, queueHistory) => queueHistory.map((item) => ({
    ...item,
    ...(items[item.uri] || {}),
  })),
);

const getLibraries = (state) => state.core.libraries;
const makeLibrarySelector = (uri) => createSelector(
  [getLibraries, getItems],
  (libraries, items) => {
    const library = libraries[uri];
    if (!library || !library.items_uris.length) return [];

    return indexToArray(items, library.items_uris);
  },
);

const getMopidySearchResults = (state, props) => (
  state.mopidy.search_results && state.mopidy.search_results[props.type]
);
const getSpotifySearchResults = (state, props) => (
  state.spotify.search_results && state.spotify.search_results[props.type]
);
const makeSearchResultsSelector = () => createSelector(
  [getMopidySearchResults, getSpotifySearchResults, getItems],
  (mopidySearchResults, spotifySearchResults, items) => {
    const uris = [
      ...mopidySearchResults || [],
      ...spotifySearchResults || [],
    ];
    return indexToArray(items, uris);
  },
);

export {
  makeItemSelector,
  makeLibrarySelector,
  makeLoadingSelector,
  makeSearchResultsSelector,
  queueHistorySelector,
};

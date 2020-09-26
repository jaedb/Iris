import { memoize } from 'lodash';
import { createSelector } from 'reselect';
import { indexToArray } from './arrays';
import { isLoading } from './helpers';

const getItems = (state) => state.core.items;
const getLoadQueue = (state) => state.ui.load_queue;
const getLibraries = (state) => state.core.libraries;

const makeItemSelector = (uri) => createSelector(
  [getItems],
  (items) => {
    if (Array.isArray(uri)) {
      return indexToArray(items, uri);
    }
    return items[uri];
  },
);
const makeLoadingSelector = (keys) => createSelector(
  [getLoadQueue],
  (loadQueue) => isLoading(loadQueue, keys),
);

const getQueueHistory = (state) => state.mopidy.queue_history;
const queueHistorySelector = createSelector(
  [getItems, getQueueHistory],
  (items, queueHistory) => queueHistory.map((item) => ({
    ...item,
    ...(items[item.uri] || {}),
  })),
);

const makeLibrarySelector = (uris) => createSelector(
  [getLibraries, getItems],
  (libraries, items) => {
    const itemUris = indexToArray(libraries, uris).reduce(
      (acc, library) => [...acc, ...library.items_uris],
      [],
    );
    return indexToArray(items, itemUris);
  },
);

const getMopidySearchResults = (state, props) => (
  state.mopidy.search_results && state.mopidy.search_results[props.type]
);
const getSpotifySearchResults = (state, props) => (
  state.spotify.search_results && state.spotify.search_results[props.type]
);
const makeSearchResultsSelector = () => createSelector(
  [getMopidySearchResults, getSpotifySearchResults, itemsSelector],
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

import { createSelector } from 'reselect';
import { indexToArray } from './arrays';
import { isLoading } from './helpers';

const getItem = (state, uri) => state.core.items[uri];
const getItems = (state) => state.core.items;
const getLoadQueue = (state) => state.ui.load_queue;
const getLibrary = (state, uri) => state.core.libraries[uri];
const getLibraries = (state) => state.core.libraries;
const getSearchResults = (state) => state.core.search_results;
const getGridGlowEnabled = (state) => state.ui.grid_glow_enabled;

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

const makeSearchResultsSelector = (term, type) => createSelector(
  [getSearchResults],
  (searchResults) => {
    if (!searchResults || searchResults.query.term !== term) return [];
    return searchResults[type] || [];
  },
);

export {
  getItem,
  getLibrary,
  getGridGlowEnabled,
  makeItemSelector,
  makeLibrarySelector,
  makeLoadingSelector,
  makeSearchResultsSelector,
  queueHistorySelector,
};

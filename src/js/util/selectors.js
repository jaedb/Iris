import { createSelector } from 'reselect';
import { indexToArray } from './arrays';
import { isLoading } from './helpers';

const getItems = (state) => state.core.items;

const makeItemSelector = (uris) => createSelector(
  [getItems],
  (items) => {
    if (Array.isArray(uris)) {
      return indexToArray(items, uris);
    }
    return items[uris];
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

export {
  makeItemSelector,
  makeLibrarySelector,
  makeLoadingSelector,
  queueHistorySelector,
};

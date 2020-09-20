import { pick } from 'lodash';
import { indexToArray } from './arrays';

const getItemFromIndex = (state, uri) => state.core.items[uri];
const getItemsFromIndex = (state, uris) => indexToArray(state.core.items, uris);

const getSearchResults = (state, provider, type, query) => {
  const {
    [provider]: {
      search_results: {
        query: resultsQuery,
        [type]: results,
      } = {},
    } = {},
  } = state;

  if (!resultsQuery) return [];
  if (resultsQuery.term !== query.term) return [];
  if (resultsQuery.type !== query.type) return [];

  if (type === 'tracks') {
    return results || [];
  }

  const selectedItems = pick(state.core.items, results);
  return Object.keys(selectedItems).length > 0 ? indexToArray(selectedItems) : [];
};

const getLibraryItems = (state, uri) => {
  const library = state.core.libraries[uri];
  if (!library || library.items_uris.length <= 0) return [];

  return getItemsFromIndex(state, library.items_uris);
}

export {
  getItemFromIndex,
  getItemsFromIndex,
  getSearchResults,
  getLibraryItems,
};

export default {
  getItemFromIndex,
  getItemsFromIndex,
  getSearchResults,
  getLibraryItems,
};

import { compact } from 'lodash';
import localForage from 'localforage';

const coreActions = require('../services/core/actions.js');
const uiActions = require('../services/ui/actions.js');

/**
 * Inspect object to check for missing dependent properties
 *
 * @param {Object} = item, dependents, fullDependents, full
 */
const getMissingDependents = ({
  item,
  dependents = [],
  fullDependents = [],
  full,
}) => {
  const allDependents = [
    ...dependents,
    ...fullDependents,
  ];
  if (!item) return allDependents;
  if (full) {
    return allDependents.filter((dep) => item[dep] === undefined || item[dep] === null);
  }
  return dependents.filter((dep) => item[dep] === undefined || item[dep] === null);
};

/**
 * Pluck all the URIs out of our dependent properties.
 *
 * @param {Object} = item, dependents, fullDependents
 */
const getDependentUris = ({
  item,
  dependents = [],
  fullDependents = [],
}) => {
  if (!item) return [];

  const getUrisOfDependent = (dep) => {
    if (!dep.match(new RegExp('(.*)_uri(.*)'))) return [];
    const uris = item[dep];

    if (uris && Array.isArray(uris)) return uris;

    return [];
  };

  return [...dependents, ...fullDependents].reduce(
    (acc, dep) => [
      ...acc,
      ...getUrisOfDependent(dep),
    ],
    [],
  );
};

/**
 * Ensure we have an item in our index
 * If it's not there, attempt to fetch it from our cold storage
 * If it's not their either, call the provided fetch()
 *
 * @param {*} Object { store, action, fetch, dependents}
 */
const ensureLoaded = ({
  store,
  containerName = 'items',
  action,
  fetch,
  dependents = [],
  fullDependents = [],
  type,
}) => {
  const {
    uri,
    options: {
      forceRefetch,
      full,
    },
  } = action;
  const {
    core: {
      [containerName]: {
        [uri]: item,
      } = {},
    } = {},
  } = store.getState();

  // Forced refetch bypasses everything
  if (forceRefetch) {
    console.info(`Force-refetching "${uri}"`);
    fetch();
    return;
  }

  const missingDependents = (itemToCheck) => getMissingDependents({
    item: itemToCheck,
    dependents,
    fullDependents,
    full,
  });

  const dependentUris = (itemToCheck) => getDependentUris({
    item: itemToCheck,
    dependents,
    fullDependents,
  });

  // Item already in our index?
  if (item) {
    if (missingDependents(item).length === 0) {
      store.dispatch(uiActions.stopLoading(uri));
      console.info(`"${uri}" already in index`);

      const uris = dependentUris(item);
      if (uris.length) {
        console.info(`Loading ${uris.length} dependents`, { uris });
        store.dispatch(coreActions.loadItems(type, uris));
      }
      return;
    }
  }

  // What about in the coldstore?
  localForage.getItem(uri).then((restoredItem) => {
    if (!restoredItem || missingDependents(restoredItem).length > 0) {
      fetch();
      return;
    }

    console.info(`Restoring "${uri}" from database`);
    store.dispatch(coreActions.restoreItemsFromColdStore([restoredItem]));

    // We already have the dependents of our restored item, so restore them.
    // We assume that because THIS item is in the coldstore, its dependents
    // are as well.
    const uris = dependentUris(restoredItem);
    if (uris.length > 0) {
      console.info(`Restoring ${uris.length} dependents from database`);

      const restoreAllDependents = uris.map(
        (dependentUri) => localForage.getItem(dependentUri),
      );
      Promise.all(restoreAllDependents).then(
        (dependentItems) => {
          store.dispatch(
            coreActions.restoreItemsFromColdStore(
              compact(dependentItems), // Squash nulls (ie items not found in coldstore)
            ),
          );
        },
      );
    }
  });
};

export {
  getMissingDependents,
  getDependentUris,
  ensureLoaded,
};

export default {
  getMissingDependents,
  getDependentUris,
  ensureLoaded,
};

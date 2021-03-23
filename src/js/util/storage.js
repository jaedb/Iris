/**
 * Storage handler
 * All localStorage tasks are handled below. This means we can detect for localStorage issues in one place
 * */

const storageFactory = (function () {
  const uid = `${new Date().getTime()}`;
  let storage;
  let result;
  try {
    (storage = window.localStorage).setItem(uid, uid);
    result = storage.getItem(uid) === uid;
    storage.removeItem(uid);
    return result && storage;
  } catch (exception) {
    return false;
  }
}());

/**
 * Get a storage value
 *
 * @param key = string
 * @param default_value = mixed (optional, if localStorage key doesn't exist, return this)
 * */
const get = (key, default_value = {}) => {
  if (storageFactory) {
    const value = storageFactory.getItem(key);
    if (value) {
      return JSON.parse(value);
    }
    return default_value;
  }
  console.warn(`localStorage not available. Using default value for '${key}'.`);
  return default_value;
};

/**
 * Set a storage value
 *
 * @param key = string
 * @param value = object
 * @param replace = boolean (optional, completely replace our local value rather than merging it)
 * */
const set = (key, value, replace = false) => {
  if (storageFactory) {
    const stored_value = storageFactory.getItem(key);

    // We have nothing to merge with, or we want to completely replace previous value
    if (!stored_value || replace) {
      var new_value = value;

      // Merge new value with existing
    } else {
      var new_value = {

        ...JSON.parse(stored_value),
        ...value,
      };
    }
    storageFactory.setItem(key, JSON.stringify(new_value));
  } else {
    console.warn(`localStorage not available. '${key}' will not perist when you close your browser.`);
  }
};

/**
 * Cache handlers
 * This allows arbritrary requests to be cached by key into local storage. Typically key would be
 * a URL, but it could also be a asset id.
 *
 * Use sparingly as localStorage is limited in size! Ideally store only the request data needed,
 * rather than the entire response data.
 */
const cache = {
  get: (key) => {
    const cache = get('cache', {});
    if (cache[`"${key}"`] !== undefined) {
      return cache[`"${key}"`];
    }
  },
  set: (key, data) => {
    const cache = get('cache', {});
    cache[`"${key}"`] = data;
    set('cache', cache);
    return true;
  },
  clear: () => {
    set('cache', {}, true);
  },
};

/**
 * Check if an image URL is cached or not
 * Useful for bypassing load animations for cached assets (eg parallax)
 *
 * @param url String
 * @return Boolean
 * */
const isCached = (url) => {
  const image = new Image();
  image.src = url;
  return image.complete;
};

export {
  get,
  set,
  cache,
  isCached,
};

export default {
  get,
  set,
  cache,
  isCached,
};

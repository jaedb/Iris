import { compact } from 'lodash';
import { uriSource } from './helpers';

/**
 * Convert an object (ie index) to an array of values
 *
 * @param index Object
 * @param keys Array of specific keys to fetch
 * @return Array
 */
const indexToArray = (index, keys) => {
  if (!index) return [];

  if (keys) {
    return compact(keys.map((key) => index[key]));
  }
  return compact(Object.keys(index).map((key) => index[key]));
};

/**
 * Digest an array of objects and pull into simple array of one property
 *
 * @param property = string
 * @param items = Array
 * @return Array
 * */
const arrayOf = (property, items = []) => {
  const array = [];
  items.forEach(
    (item) => {
      if (item[property] === undefined) return;
      if (item[property] === null) return;
      array.push(item[property]);
    },
  );
  return array;
};


/**
 * Merge duplicated items in an array
 *
 * @param list Array the unclean array
 * @param key string = the unique key (id, uri, tlid, etc)
 * */
const mergeDuplicates = function (list, key) {
  const clean_list = [];
  const keyed_list = {};

  for (var i in list) {
    let item = list[i];
    if (item[key] in keyed_list) {
      item = { ...keyed_list[item[key]], ...item };
    }
    keyed_list[item[key]] = item;
  }

  for (i in keyed_list) {
    clean_list.push(keyed_list[i]);
  }

  return clean_list;
};


/**
 * Remove duplicate items in a simple array
 *
 * @param list Array the unclean array
 * */
const removeDuplicates = function (array) {
  const unique = [];

  for (const i in array) {
    if (unique.indexOf(array[i]) <= -1) {
      unique.push(array[i]);
    }
  }

  return unique;
};


/**
 * Apply a partial text search on an array of objects
 *
 * @param field = string (the field we're to search)
 * @param value = string (the value to find)
 * @param array = array of objects to search
 * @param singular = boolean (just return the first result)
 * @return array
 * */
const applyFilter = function (field, value, array, singular = false) {
  const results = [];

  for (let i = 0; i < array.length; i++) {
    if (array[i][field] && String(array[i][field]).toLowerCase().includes(String(value).toLowerCase())) {
      if (singular) {
        return array[i];
      }
      results.push(array[i]);
    }
  }

  return results;
};


/**
 * Convert a list of indexes to a useable range
 * We ignore stragglers, and only attend to the first 'bunch' of consecutive indexes
 *
 * @param indexes array of int
 * */
const createRange = function (indexes) {
  // sort our indexes smallest to largest
  function sortAsc(a, b) {
    return a - b;
  }
  indexes.sort(sortAsc);

  // iterate indexes to build the first 'bunch'
  const first_bunch = [];
  let previous_index = false;
  for (let i = 0; i < indexes.length; i++) {
    if (!previous_index || previous_index == indexes[i] - 1) {
      first_bunch.push(indexes[i]);
      previous_index = indexes[i];
    }
    // TODO: break when we find an integer step for better performance
  }

  return {
    	start: first_bunch[0],
    	length: first_bunch.length,
  };
};


/**
 * Sort an array of objects
 * @param array = array to sort
 * @param property = string to sort by
 * @param reverse = boolean
 * @param sort_map = array of value ordering (rather than alphabetical, numerical, etc)
 * @return array
 * */
const sortItems = function (array, property, reverse = false, sort_map = null) {
  if (!array || array.length <= 0) {
    return [];
  }

  function get_value(value) {
    const split = property.split('.');
    for (const property_element of split) {
      // Apply sort on a property of the first item of an array
      if (property_element == 'first') {
        if (Array.isArray(value) && value.length > 0) {
          value = value[0];
          continue;
        } else {
          return null;
        }

        // Just need the length of an array
      } else if (property_element == 'length') {
        if (Array.isArray(value)) {
          return value.length;
        }
        return 0;


        // No value here
      } else if (typeof (value[property_element]) === 'undefined') {
        return null;
      }

      // Otherwise continue looping to the end of the split property
      value = value[property_element];
    }

    return value;
  }

  function compare(a, b) {
    let a_value = get_value(a);
    let b_value = get_value(b);

    // Sorting by URI as a reference for sorting by uri source (first component of URI)
    if (property === 'uri') {
      a_value = uriSource(a_value);
      b_value = uriSource(b_value);
    }

    // Map sorting
    // Use the index of the string as a sorting mechanism
    if (sort_map) {
      const a_index = sort_map.indexOf(`${a_value}:`);
      const b_index = sort_map.indexOf(`${b_value}:`);
      if (a_index < b_index) return 1;
      if (a_index > b_index) return -1;

      // Boolean sorting
    } else if (typeof a_value === 'boolean' && typeof b_value === 'boolean') {
      if (a_value && !b_value) return -1;
      if (!a_value && b_value) return 1;

      // Numeric sorting
    } else if (typeof a_value === 'number' && typeof b_value === 'number') {
      if (a_value == null && b_value == null) return 0;
      if (a_value == null) return -1;
      if (b_value == null) return 1;
      if (parseInt(a_value) > parseInt(b_value)) return 1;
      if (parseInt(a_value) < parseInt(b_value)) return -1;

      // Alphabetic sorting
    } else {
      if (a_value && !b_value) return -1;
      if (!a_value && b_value) return 1;
      if (!a_value && !b_value) return 0;
      if (a_value.toLowerCase() > b_value.toLowerCase()) return 1;
      if (a_value.toLowerCase() < b_value.toLowerCase()) return -1;
    }
    return 0;
  }

  const sorted = Object.assign([], array.sort(compare));
  if (reverse) {
    sorted.reverse();
  }
  return sorted;
};

/**
 * Shuffle items in place
 *
 * @param Array items
 * @return Array
 * */
const shuffle = function (array) {
  let j; let x; let
    i;
  for (i = array.length - 1; i > 0; i--) {
    j = Math.floor(Math.random() * (i + 1));
    x = array[i];
    array[i] = array[j];
    array[j] = x;
  }
  return array;
};

export {
  arrayOf,
  mergeDuplicates,
  removeDuplicates,
  applyFilter,
  createRange,
  sortItems,
  shuffle,
  indexToArray,
};

export default {
  arrayOf,
  mergeDuplicates,
  removeDuplicates,
  applyFilter,
  createRange,
  sortItems,
  shuffle,
  indexToArray,
};

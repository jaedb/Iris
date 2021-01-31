import { indexToArray, arrayOf } from "./arrays";
import { encodeUri } from "./format";

/**
 * Returns a function, that, as long as it continues to be invoked, will not
 * be triggered. The function will be called after it stops being called for
 * N milliseconds. If `immediate` is passed, trigger the function on the
 * leading edge, instead of the trailing.
 * */
const debounce = function (fn, wait, immediate) {
  let timeout;
  return function () {
    const context = this;
    const
      args = arguments;

    const later = function () {
      timeout = null;
      if (!immediate) {
        fn.apply(context, args);
      }
    };

    const callNow = immediate && !timeout;
    clearTimeout(timeout);
    timeout = setTimeout(later, wait);

    if (callNow) {
      fn.apply(context, args);
    }
  };
};


const throttle = function (fn, delay) {
  let lastCall = 0;
  return function (...args) {
    const now = (new Date()).getTime();
    if (now - lastCall < delay) {
      return;
    }
    lastCall = now;
    return fn(...args);
  };
};




/**
 * Set the app's favicon to a specific image.
 *
 * @param filename String
 */
const setFavicon = function (filename) {
  const links = document.getElementsByClassName('favicon');
  for (const link of links) {
    // Construct new <links>
    const new_link = document.createElement('link');
    new_link.className = link.className;
    new_link.rel = link.rel;
    new_link.href = `/iris/assets/${filename}`;
    if (link.type) {
      new_link.type = link.type;
    }

    // Remove the old one and add the new one
    document.head.removeChild(link);
    document.head.appendChild(new_link);
  }
};


/**
 * Digest a react-router's location.search string into an array of values
 *
 * @param key String = the key you want from the URL
 * @param string String = the locaion.search string
 */
const queryString = (key, string, compact = true) => {
  const elements = string.replace('?', '').split('&');
  const results = elements.reduce((accumulator, current) => {
    const subElements = current.split('=');
    let results = [];

    // We decode the URI, but also treat "+" as a space. This is needed for backend CGI.encode that
    // happens when redirecting from an OAuth failure.
    if (subElements[0] === key) {
      results = subElements[1].split(',').map(
        item => decodeURIComponent(item.replace(/\+/g, '%20')),
      );
    }
    return [...accumulator, ...results];
  }, []);

  if (compact && results.length === 1) return results[0];
  if (compact && results.length === 0) return null;
  return results;
};



const generateGuid = function (type = 'numeric', length = 12) {
  // numeric
  if (type == 'numeric') {
    const date = new Date().valueOf().toString();
    const random_number = Math.floor((Math.random() * 100)).toString();
    return parseInt(date + random_number);
  }
  const format = 'x'.repeat(length);
  return format.replace(/[xy]/g, (c) => {
    const r = Math.random() * 16 | 0;
    const
      v = c == 'x' ? r : (r & 0x3 | 0x8);
    return v.toString(length);
  });
};

const getCurrentPusherConnection = function (connections, connectionid) {
  function isCurrentConnection(connection) {
    return connection.connectionid == newProps.pusher.connectionid;
  }

  const currentConnection = newProps.pusher.connections.find(isCurrentConnection);
  if (!currentConnection) return false;

  return currentConnection;
};

/**
 * Figure out a URI's source namespace
 * @param uri = string
 * */
let uriSource = function (uri) {
  if (!uri) return '';

  const exploded = `${uri}`.split(':');
  return exploded[0];
};
/**
 * Identify what kind of asset a URI is (playlist, album, etc)
 *
 * @param uri = string
 * @return string
 * */
const uriType = function (uri) {
  if (!uri) return '';

  const exploded = `${uri}`.split(':');

  if (exploded[0] === 'm3u') {
    return 'playlist';
  }

  if (exploded[0] === 'youtube') {
    const youtubeParts = exploded[1].split('/');
    if (youtubeParts[0] === 'video') {
      return 'track';
    }
    return youtubeParts[0];
  }

  if (exploded[0] === 'iris') {
    return exploded[1];
  }

  if (exploded[0] === 'dleyna') {
    return 'track';
  }

  if (exploded[0] === 'tunein') {
    switch (exploded[1]) {
      case 'station':
        return 'album';
      default:
        return exploded[1];
    }
  }

  switch (exploded[1]) {
    case 'library':
      return exploded[2];
    case 'track':
    case 'artist':
    case 'album':
    case 'playlist':
    case 'genre':
      return exploded[1];
    case 'user':
      if (exploded.length > 3 && exploded[3] === 'playlist') {
        return 'playlist';
      }
      return exploded[1];
    default:
      return exploded[1];
  }
};

const sourceIcon = function (uri, source = null) {
  if (uri) source = uriSource(uri);
  switch (source) {
    case 'local':
    case 'm3u':
    case 'file':
      return 'folder';

    case 'gmusic':
      return 'google';

    case 'podcast':
    case 'podcast+file':
    case 'podcast+http':
    case 'podcast+https':
    case 'podcast+itunes':
      return 'podcast';

    case 'tunein':
    case 'somafm':
    case 'dirble':
      return 'cloud';

    case 'spotify':
    case 'soundcloud':
    case 'lastfm':
    case 'youtube':
    case 'tidal':
      return source;

    default:
      return 'cloud';
  }
};

/**
 * Get an element from a URI
 * @param element = string, the element we wish to extract
 * @param uri = string
 * */
const getFromUri = (element, uri) => {
  if (!uri) return null;
  const exploded = `${uri}`.split(':');

  switch (element) {
    case 'mbid':
      const index = exploded.indexOf('mbid');
      if (index > -1) return exploded[index + 1];
      break;

    case 'artistid':
      if (exploded[1] == 'artist') {
        return exploded[2];
      }
      break;

    case 'albumid':
      if (exploded[1] == 'album') {
        return exploded[2];
      }
      break;

    case 'playlistid':
      if (exploded[1] == 'playlist') {
        return exploded[2];
      }
      if (exploded[1] == 'user' && exploded[3] == 'playlist') {
        return exploded[4];
      }
      break;

    case 'playlistowner':
      if (exploded[1] == 'user' && exploded[3] == 'playlist') {
        return exploded[2];
      }
      break;

    case 'trackid':
      if (exploded[1] == 'track') {
        return exploded[2];
      }
      if (exploded[0] === 'dleyna') {
        return uri.replace('dleyna:', '');
      }
      break;

    case 'userid':
      if (exploded[1] == 'user') {
        return exploded[2];
      }
      break;

    case 'genreid':
      if (exploded[1] == 'genre') {
        return exploded[2];
      }
      break;

    case 'categoryid':
      if (exploded[1] == 'category') {
        return exploded[2];
      }
      break;

    case 'seeds':
      if (exploded[1] == 'discover') {
        return exploded[2];
      } else if (exploded[1] === 'radio') {
        const seeds = exploded[2].split(',');
        return seeds.map(seed => seed.replace(/_/gi, ':'));
      }
      break;

    case 'searchtype':
      if (exploded[1] == 'search') {
        return exploded[2];
      }
      break;

    case 'searchterm':
      if (exploded[1] == 'search') {
        return exploded[3];
      }
      break;

    default:
      return null;
  }
};

/**
 * Build a link to an asset. Using the URI type we can ascertain where we need
 * to direct the user (eg /track/local:track:1235.mp3)
 *
 * @param $uri = String
 * @param $type = String, optional
 * @return String
 * */
const buildLink = (uri, type = null) => {
  let link = `/${type || uriType(uri)}/`;

  // Encode the whole URI as though it's a component. This makes it URL friendly for
  // all Mopidy backends (some use URIs like local:track:http://rss.com/stuff.mp3) which
  // is never going to work nicely.
  link += encodeUri(uri);
  return link;
};

/**
 * Detect touch-ability
 */
const isTouchDevice = function () {
  return 'ontouchstart' in document.documentElement;
};

/**
 * Figure out if a value is a number
 * @param value = mixed
 * @return boolean
 * */
const isNumeric = function (value) {
  return !isNaN(parseFloat(value)) && isFinite(value);
};


/**
 * Figure out if a value is an object
 * @param value = mixed
 * @return boolean
 * */
let isObject = function (value) {
  return value instanceof Object && value.constructor === Object;
};

/**
 * Convert an array of strings to an array of RegExp objects
 * 
 * @param {Array} keys
 */
const toRegExp = function (keys) {
  return keys.map((key) => {
    try {
      return new RegExp(key);
    } catch {
      // Fucks with unit tests, but helpful for debugging.
      // console.error('Could not convert string to RegEx', key);
      return null;
    }
  });
};

/**
 * Detect if an item is in the loading queue. We simply loop all load items to
 * see if any load queue keys match our 'includes' expression AND our 'excludes' expression(s)
 *
 * @param {Object} load_queue (passed from store)
 * @param {Array} keys array of regex strings
 * @return {Boolean}
 * */
const isLoading = function (load_queue = {}, keys = []) {
  if (!load_queue || !keys) return false;

  const expressions = toRegExp(keys);
  const queue = indexToArray(load_queue);

  const matches = queue.filter((qk) => {
    const matchingExpressions = keys.filter((exp) => qk.match(exp));

    return (matchingExpressions.length === expressions.length);
  });

  return matches.length > 0;
};

/**
 * Is this app running from the hosted instance?
 * For example the GitHub-hosted UI
 *
 * @param Array hosts = valid hosted domain names
 * @return Boolean
 * */
const isHosted = function (hosts = ['jaedb.github.io']) {
  const {
    hostname
  } = window.location;
  return hosts.includes(hostname);
};


/**
 * Get indexed record(s) by URI from our asset index
 *
 * @param store = obj
 * @param uris = mixed (array or string)
 * @return array
 * */
const getIndexedRecords = function (index, uris) {
  const records = [];

  // Wrap in array, if we've only got one URI
  if (!(uris instanceof Array)) {
    uris = [uris];
  }

  for (let i = 0; i < uris.length; i++) {
    if (index.hasOwnProperty(uris[i])) {
      records.push(index[uris[i]]);
    }
  }

  return records;
};


/**
 * Uppercase-ify the first character of a string
 *
 * @param string String
 * @return String
 * */
const titleCase = function (string = '') {
  return string.charAt(0).toUpperCase() + string.slice(1);
};

/**
 * Use a keyword to return an icon name
 */
const iconFromKeyword = (name) => {
  const iconWords = [{
      icon: 'business',
      words: ['office', 'work']
    },
    {
      icon: 'king_bed',
      words: ['bed']
    },
    {
      icon: 'weekend',
      words: ['lounge', 'tv', 'sitting room']
    },
    {
      icon: 'directions_car',
      words: ['garage', 'basement']
    },
    {
      icon: 'local_laundry_service',
      words: ['laundry']
    },
    {
      icon: 'fitness_center',
      words: ['gym']
    },
    {
      icon: 'kitchen',
      words: ['kitchen']
    },
    {
      icon: 'deck',
      words: ['deck', 'outside']
    },
    {
      icon: 'restaurant_menu',
      words: ['dining', 'dinner']
    },
    {
      icon: 'laptop',
      words: ['laptop']
    },
    {
      icon: 'bug_report',
      words: ['test', 'debug']
    },
    {
      icon: 'child_care',
      words: ['kids', 'baby']
    },
    {
      icon: 'smartphone',
      words: ['phone', 'mobile']
    },
    {
      icon: 'web',
      words: ['web', 'browser', 'iris']
    },
  ];
  for (let item of iconWords) {
    for (let word of item.words) {
      if (name.match(new RegExp(`(${word})`, 'gi'))) {
        return item.icon;
      }
    }
  };
}

/**
 * Scroll to the top of the page
 * Our 'content' is housed in the <main> DOM element
 * We make sure the target supports scrolling before we attempt it
 * Safari and IE Edge, well, don't.
 *
 * @param target String (element ID, optional)
 * @param smooth_scroll Boolean (optional)
 * */
const scrollTo = function (target = null, smooth_scroll = false) {
  const main = document.getElementById('main');

  // Remove our smooth-scroll class
  if (!smooth_scroll) {
    main.classList.remove('smooth-scroll');
  }

  // Target is a number, so treat as pixel position
  if (target && Number.isInteger(target)) {
    if (typeof main.scrollTo === 'function') {
      main.scrollTop = target;
    }


    // Target is a string representing a DOM element by class/id
  } else if (target) {
    let element = null;

    if (target.charAt(0) == '#') {
      element = document.getElementById(target.substring(1));
    } else if (target.charAt(0) == '.') {
      element = document.getElementsByClassName(target.substring(1));
      if (element.length > 0) {
        element = element[0];
      }
    } else {
      console.error(`Invalid target type '${target}'. Must start with '#' or '.'.`);
    }

    if (element && typeof element.scrollIntoView === 'function') {
      element.scrollIntoView();
    }
  } else {
    main.scrollTop = 0;
  }

  // Now reinstate smooth scroll
  if (!smooth_scroll) {
    main.classList.add('smooth-scroll');
  }
};


/**
 * Upgrade one or many Spotify Playlist URIs
 * This is their new, simplified syntax (September 2018) but they haven't updated it everywhere
 * So we need to manually strip user:abc to keep things consistent
 *
 * @param uris Array|String
 * @return Array|String
 * */
const upgradeSpotifyPlaylistUris = function (uris) {
  const upgraded = [];

  for (let uri of uris) {
    if (uri.includes('spotify:user:')) {
      uri = uri.replace(/spotify:user:([^:]*?):/i, 'spotify:');
    }
    upgraded.push(uri);
  }

  return upgraded;
};

// As above, but for a single URI
const upgradeSpotifyPlaylistUri = function (uri) {
  return upgradeSpotifyPlaylistUris([uri])[0];
};

/**
 * Decode and encode Mopidy playlist URIs
 * This is needed as Mopidy encodes *some* characters in playlist URIs (but not other characters)
 * We need to retain ":" because this a reserved URI separator
 */
const decodeMopidyUri = (uri, decodeComponent = true) => {
  let decoded = decodeComponent ? decodeURIComponent(uri) : uri;
  decoded = decoded.replace(/\s/g, '%20'); // space
  decoded = decoded.replace(/\[/g, '%5B'); // [
  decoded = decoded.replace(/\]/g, '%5D'); // ]
  decoded = decoded.replace(/\(/g, '%28'); // (
  decoded = decoded.replace(/\)/g, '%29'); // )
  decoded = decoded.replace(/\#/g, '%23'); // #
  return decoded;
};

const encodeMopidyUri = (uri, encodeComponent = true) => {
  let encoded = encodeComponent ? encodeURIComponent(uri) : uri;
  encoded = encoded.replace(/\%20/g, ' '); // space
  encoded = encoded.replace(/\%5B/g, '['); // [
  encoded = encoded.replace(/\%5D/g, ']'); // ]
  encoded = encoded.replace(/\%28/g, '('); // (
  encoded = encoded.replace(/\%29/g, ')'); // )
  encoded = encoded.replace(/\%23/g, '#'); // #
  return encoded;
};

export {
  debounce,
  throttle,
  setFavicon,
  queryString,
  generateGuid,
  getCurrentPusherConnection,
  uriSource,
  uriType,
  sourceIcon,
  getFromUri,
  buildLink,
  isTouchDevice,
  isNumeric,
  isObject,
  isLoading,
  isHosted,
  getIndexedRecords,
  titleCase,
  scrollTo,
  upgradeSpotifyPlaylistUris,
  upgradeSpotifyPlaylistUri,
  iconFromKeyword,
  decodeMopidyUri,
  encodeMopidyUri,
};

export default {
  debounce,
  throttle,
  setFavicon,
  queryString,
  generateGuid,
  getCurrentPusherConnection,
  uriSource,
  uriType,
  sourceIcon,
  getFromUri,
  buildLink,
  isTouchDevice,
  isNumeric,
  isObject,
  isLoading,
  isHosted,
  getIndexedRecords,
  titleCase,
  scrollTo,
  upgradeSpotifyPlaylistUris,
  upgradeSpotifyPlaylistUri,
  iconFromKeyword,
  decodeMopidyUri,
  encodeMopidyUri,
};
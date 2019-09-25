
export const isTouchDevice = function () {
  return 'ontouchstart' in document.documentElement;
};

/**
 * Returns a function, that, as long as it continues to be invoked, will not
 * be triggered. The function will be called after it stops being called for
 * N milliseconds. If `immediate` is passed, trigger the function on the
 * leading edge, instead of the trailing.
 * */
export const debounce = function (fn, wait, immediate) {
  let timeout;
  return function () {
    const context = this; const
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


export const throttle = function (fn, delay) {
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
 * Storage handler
 * All localStorage tasks are handled below. This means we can detect for localStorage issues in one place
 * */

const storage = (function () {
  const uid = new Date();
  let storage;
  let result;
  try {
    (storage = window.localStorage).setItem(uid, uid);
    result = storage.getItem(uid) == uid;
    storage.removeItem(uid);
    return result && storage;
  } catch (exception) {}
}());

/**
 * Get a storage value
 *
 * @param key = string
 * @param default_value = mixed (optional, if localStorage key doesn't exist, return this)
 * */
export const getStorage = function (key, default_value = {}) {
  if (storage) {
    const value = storage.getItem(key);
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
export const setStorage = function (key, value, replace = false) {
  if (storage) {
    const stored_value = storage.getItem(key);

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
    storage.setItem(key, JSON.stringify(new_value));
  } else {
    console.warn(`localStorage not available. '${key}'' will not perist when you close your browser.`);
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
export const cache = {
  get: key => {
    const cache = getStorage('cache', {});
    if (cache[`"${key}"`] !== undefined) {
      return cache[`"${key}"`];
    }
  },
  set: (key, data) => {
    let cache = getStorage('cache', {});
    cache[`"${key}"`] = data;
    setStorage('cache', cache);
    return true;
  },
  clear: () => {
    setStorage('cache', {}, true);
  }
}


/**
 * Convert a string to JSON, after we've checked whether it needs
 * conversion or not.
 *
 * @param data String or Object
 * @return Object
 * */
export const toJSON = function (data) {
  // Parse it
  try {
    const json = JSON.parse(data);
    return json;

    // Could not parse string
  } catch (e) {
    // Check if it's JSON already
    if (data.constructor === {}.constructor) {
      return data;
    }
    console.error('Could not convert non-JSON', string);
  }
  return {};
};


/**
 * Set the app's favicon to a specific image.
 *
 * @param filename String
 */
export const setFavicon = function (filename) {
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
 * Check if an image URL is cached or not
 * Useful for bypassing load animations for cached assets (eg parallax)
 *
 * @param url String
 * @return Boolean
 * */
export const isCached = function (url) {
  const image = new Image();
  image.src = url;
  return image.complete;
};


/**
 * Digest an array of Mopidy image objects into a universal format. We also re-write
 * image URLs to be absolute to the mopidy server (required for proxy setups).
 *
 * @param mopidy = obj (mopidy store object)
 * @param images = array
 * @return array
 * */
export const digestMopidyImages = function (mopidy, images) {
  const digested = [];

  for (let i = 0; i < images.length; i++) {
    // Image object (ie from images.get)
    if (typeof images[i] === 'object') {
      // Accommodate backends that provide URIs vs URLs
      let { url } = images[i];
      if (!url && images[i].uri) {
        url = images[i].uri;
      }

	        // Amend our URL
	        images[i].url = url;

      // Replace local images to point directly to our Mopidy server
	        if (url && url.startsWith('/images/')) {
	            url = `//${mopidy.host}:${mopidy.port}${url}`;
	        }

	    // String-based image
    } else if (typeof images[i] === 'string') {
      // Replace local images to point directly to our Mopidy server
	        if (images[i].startsWith('/images/')) {
	            images[i] = `//${mopidy.host}:${mopidy.port}${images[i]}`;
	        }
    }

    digested.push(images[i]);
  }

  return digested;
};


export const generateGuid = function (type = 'numeric') {
  // numeric
  if (type == 'numeric') {
    const date = new Date().valueOf().toString();
    const random_number = Math.floor((Math.random() * 100)).toString();
    return parseInt(date + random_number);
  }
  const format = 'xxxxxxxxxx';
  return format.replace(/[xy]/g, (c) => {
    const r = Math.random() * 16 | 0; const
      v = c == 'x' ? r : (r & 0x3 | 0x8);
    return v.toString(16);
  });
};

export const getCurrentPusherConnection = function (connections, connectionid) {
  function isCurrentConnection(connection) {
    return connection.connectionid == newProps.pusher.connectionid;
  }

  const currentConnection = newProps.pusher.connections.find(isCurrentConnection);
  if (!currentConnection) return false;

  return currentConnection;
};


/**
 * Get a track's icon
 * @param track object
 * @return string
 * */
export const getTrackIcon = function (current_track = false, core = false) {
  if (!core) return false;
  if (!current_track) return false;
  if (typeof (current_track.uri) === 'undefined') return false;
  if (typeof (core.tracks[current_track.uri]) === 'undefined') return false;
  const track = core.tracks[current_track.uri];
  if (!track.images) return false;
  return formatImages(track.images).small;
};

/**
 * Format image URLs into a consistent size-based object
 * We digest all our known image source formats into a universal small,medium,large,huge object
 *
 * @param $data mixed
 * @return Object
 * */
export let formatImages = function (data) {
  const sizes = {
    formatted: true,
    small: null,
    medium: null,
    large: null,
    huge: null,
  };

  if (!data) {
    return sizes;
  }

  // An array of images has been provided
  if (Array.isArray(data)) {
    if (data.length <= 0) {
      return sizes;
    }

    for (let i = 0; i < data.length; i++) {
      const image = data[i];

      // Already-formatted
      if (image.formatted) {
        return image;

        // Mopidy image object
      } if (image.__model__ && image.__model__ == 'Image') {
        if (image.width < 400) {
          sizes.small = image.url;
        } else if (image.width < 800) {
          sizes.medium = image.url;
        } else if (image.width < 1000) {
          sizes.large = image.url;
        } else {
          sizes.huge = image.url;
        }

        // Mopidy image string
      } else if (typeof (image) === 'string') {
        sizes.small = image;

        // spotify-styled images
      } else if (image.width !== undefined) {
        if (image.width < 400) {
          sizes.small = image.url;
        } else if (image.width < 800) {
          sizes.medium = image.url;
        } else if (image.width < 1000) {
          sizes.large = image.url;
        } else {
          sizes.huge = image.url;
        }

        // lastfm-styled images
      } else if (image.size !== undefined) {
        switch (image.size) {
          case 'mega':
          case 'extralarge':
          case 'large':
            sizes.medium = image['#text'];
            break;
          case 'medium':
          case 'small':
            sizes.small = image['#text'];
            break;
        }
      }
    }

    // An object of images has been provided
    // The Genius avatar object is an example of this
  } else {
    if (data.small) sizes.small = data.small.url;
    if (data.medium) sizes.medium = data.medium.url;
    if (data.large) sizes.large = data.large.url;
    if (data.huge) sizes.huge = data.huge.url;
  }

  // Inherit images where we haven't been given the appropriate size
  // Ie small duplicated to tiny, large duplicated to medium, etc
  if (!sizes.small) {
    if (sizes.medium) sizes.small = sizes.medium;
    else if (sizes.large) sizes.small = sizes.large;
    else if (sizes.huge) sizes.small = sizes.huge;
    else sizes.small = null;
  }
  if (!sizes.medium) {
    if (sizes.large) sizes.medium = sizes.large;
    else if (sizes.huge) sizes.medium = sizes.huge;
    else sizes.medium = sizes.small;
  }
  if (!sizes.large) sizes.large = sizes.medium;
  if (!sizes.huge) sizes.huge = sizes.large;

  return sizes;
};


/**
 * Format a simple object
 * This is a shell record containing only the bare essentials. Typically
 * a tracks' artists/album
 *
 * @param data obj
 * @return obj
 * */
export const formatSimpleObject = function (data) {
  const simple_object = {};
  const fields = [
    'uri',
    'name',
  ];

  for (const field of fields) {
    if (data.hasOwnProperty(field)) {
      simple_object[field] = data[field];
    }
  }

  return simple_object;
};

/**
 * Format multiple items
 *
 * @param tracks Array
 * @return Array
 * */
export const formatTracks = function (records = []) {
  const formatted = [];
  for (const record of records) {
	    formatted.push(formatTrack(record));
  }
  return formatted;
};
export const formatAlbums = function (records = []) {
  const formatted = [];
  for (const record of records) {
	    formatted.push(formatAlbum(record));
  }
  return formatted;
};
export const formatArtists = function (records = []) {
  const formatted = [];
  for (const record of records) {
	    formatted.push(formatArtist(record));
  }
  return formatted;
};
export const formatPlaylists = function (records = []) {
  const formatted = [];
  for (const record of records) {
	    formatted.push(formatTrack(record));
  }
  return formatted;
};
export const formatUsers = function (records = []) {
  const formatted = [];
  for (const record of records) {
	    formatted.push(formatUser(record));
  }
  return formatted;
};


/**
 * Format our album objects into a universal format
 *
 * @param data obj
 * @return obj
 * */
export let formatAlbum = function (data) {
  const album = {};
  const fields = [
    'uri',
    'provider',
    'name',
    'type',
    'added_at',
    'release_date',
    'listeners',
    'play_count',
    'wiki',
    'wiki_publish_date',
    'popularity',
    'images',
    'artists_uris',
    'tracks_uris',
    'artists',	// Array of simple records
  ];

  // Loop fields and import from data
  for (const field of fields) {
    if (data.hasOwnProperty(field)) {
      album[field] = data[field];
    }
  }

  if (album.images && !album.images.formatted) {
    album.images = formatImages(album.images);
  }

  if (data.date && !album.date) {
    album.release_date = data.date;
  }
  if (data.album_type) {
    album.type = data.album_type;
  }
  if (album.provider === undefined && album.uri !== undefined) {
    album.provider = uriSource(album.uri);
  }

  return album;
};


/**
 * Format our artist objects into a universal format
 *
 * @param data obj
 * @return obj
 * */
export let formatArtist = function (data) {
  const artist = {};
  const fields = [
    'uri',
    'provider',
    'mbid',
    'name',
    'type',
    'popularity',
    'followers',
    'listeners',
    'added_at',
    'biography',
    'biography_link',
    'biography_publish_date',
    'related_artists_uris',
    'albums_uris',
    'albums_total',
    'albums_more',
    'tracks_uris',
    'tracks_total',
    'tracks_more',
  ];

  // Loop fields and import from data
  for (const field of fields) {
    if (data.hasOwnProperty(field)) {
      artist[field] = data[field];
    }
  }

  if (data.images && data.images.length > 0) {
    artist.images = [formatImages(data.images)];
  }

  if (data.followers && data.followers.total !== undefined) {
    artist.followers = data.followers.total;
  }

  if (data.bio) {
    if (data.bio.content && !artist.biography) {
      artist.biography = data.bio.content;
    }
    if (data.bio.links && data.bio.links.link && data.bio.links.link.href && !artist.biography_link) {
      artist.biography_link = data.bio.links.link.href;
    }
    if (data.bio.published && !artist.biography_publish_date) {
      artist.biography_publish_date = data.bio.published;
    }
  }

  if (artist.provider === undefined && artist.uri !== undefined) {
    artist.provider = uriSource(artist.uri);
  }

  return artist;
};


/**
 * Format our playlist objects into a universal format
 *
 * @param data obj
 * @return obj
 * */
export const formatPlaylist = function (data) {
  const playlist = {};
  const fields = [
    'uri',
    'snapshot_id',
    'provider',
    'type',
    'collaborative',
    'public',
    'name',
    'description',
    'images',
    'popularity',
    'followers',
    'added_at',
    'last_modified_date',
    'can_edit',
    'owner',
    'user_uri',
    'tracks_uris',
    'tracks_total',
    'tracks_more',
  ];

  // Loop fields and import from data
  for (const field of fields) {
    if (data.hasOwnProperty(field)) {
      playlist[field] = data[field];
    }
  }

  if (playlist.images && !playlist.images.formatted) {
    playlist.images = formatImages(playlist.images);
  }

  if (data.followers && data.followers.total !== undefined) {
    playlist.followers = data.followers.total;
  }

  if (data.tracks && data.tracks.total !== undefined) {
    playlist.tracks_total = data.tracks.total;
  }

  if (data.owner) {
    playlist.owner = {
      id: data.owner.id,
      uri: data.owner.uri,
      name: (data.owner.display_name ? data.owner.display_name : null),
    };
    playlist.user_uri = data.owner.uri;
  }

  // Spotify upgraded their playlists URI to remove user component (Sept 2018)
  playlist.uri = upgradeSpotifyPlaylistUri(playlist.uri);

  if (playlist.provider === undefined && playlist.uri !== undefined) {
    playlist.provider = uriSource(playlist.uri);
  }

  return playlist;
};


/**
 * Format a user objects into a universal format
 *
 * @param data obj
 * @return obj
 * */
export let formatUser = function (data) {
  const user = {};
  const fields = [
    'id',
    'uri',
    'provider',
    'name',
    'images',
    'followers',
    'playlists_uris',
    'playlists_total',
    'playlists_more',
  ];

  // Loop fields and import from data
  for (const field of fields) {
    if (data.hasOwnProperty(field)) {
      user[field] = data[field];
    }
  }

  if (!user.images && data.image) {
    user.images = formatImages(data.image);
  } else if (!user.images && data.avatar) {
    user.images = formatImages(data.avatar);
  } else if (user.images && !user.images.formatted) {
    user.images = formatImages(user.images);
  }

  if (data.followers && data.followers.total !== undefined) {
    user.followers = data.followers.total;
  }
  if (data.realname) {
    user.name = data.realname;
  }
  if (data.display_name && !user.name) {
    user.name = data.display_name;
  }
  if (data.id && !user.name) {
    user.name = data.id;
  }
  if (user.provider === undefined && user.uri !== undefined) {
    user.provider = uriSource(user.uri);
  }

  return user;
};


/**
 * Format tracks into our universal format
 *
 * @param data obj
 * @return obj
 * */
export let formatTrack = function (data) {
  const track = {};
  const fields = [
    'uri',
    'tlid',
    'provider',
    'name',
    'images',
    'release_date',
    'disc_number',
    'track_number',
    'duration',
    'followers',
    'popularity',
    'userloved',
    'is_explicit',
    'is_local',
    'lyrics',
    'lyrics_path',
    'lyrics_results',
    'artists',	// Array of simple records
    'album',		// Array of simple records
  ];

  // Nested track object (eg in spotify playlist)
  if (data && data.track && isObject(data.track)) {
    // Copy wrapper's details (if applicable)
    if (data.added_by) {
      data.track.added_by = data.added_by;
    }
    if (data.added_at) {
      data.track.added_at = data.added_at;
    }
    if (data.tlid) {
      data.track.tlid = data.tlid;
    }

    // And now flatten
    data = data.track;
  }

  // Loop fields and import from data
  for (const field of fields) {
    if (data.hasOwnProperty(field)) {
      track[field] = data[field];
    }
  }

  if (data.followers && data.followers.total) {
    track.followers = data.followers.total;
  }

  if (track.duration === undefined && data.duration_ms !== undefined) {
    track.duration = data.duration_ms;
  } else if (track.duration === undefined && data.length !== undefined) {
    track.duration = data.length;
  }

  if (track.track_number === undefined && data.track_no !== undefined) {
    	track.track_number = data.track_no;
  }

  if (track.disc_number === undefined && data.disc_no !== undefined) {
    	track.disc_number = data.disc_no;
  }

  if (track.release_date === undefined && data.date !== undefined) {
    	track.release_date = data.date;
  }

  if (track.explicit === undefined && data.explicit !== undefined) {
    	track.is_explicit = data.explicit;
  }

  // Copy images from albums (if applicable)
  // TOOD: Identify if we stil need this...
  if (data.album && data.album.images) {
    	if (track.images === undefined || !track.images.formatted) {
    		track.images = formatImages(data.album.images);
    	}
  }

  if (track.provider === undefined && track.uri !== undefined) {
    track.provider = uriSource(track.uri);
  }

  return track;
};

/**
 * Format a snapcast client object into a universal format
 *
 * @param data obj
 * @return obj
 * */
export const formatClient = function (data) {
  const client = {};
  const fields = [
    'id',
    'connected',
    'name',
    'host_name',
    'volume',
    'mute',
    'latency',
    'power_on_command',
    'power_off_command',
  ];

  for (const field of fields) {
    if (data.hasOwnProperty(field)) {
      client[field] = data[field];
    }
  }

  if (data.config) {
    if (client.latency === undefined && data.config.latency !== undefined) {
      client.latency = data.config.latency;
    }

    if (!client.name && data.config.name) {
      client.name = data.config.name;
    }

    if (data.config.volume) {
      if (data.config.volume.percent) {
        client.volume = data.config.volume.percent;
      }
      if (data.config.volume.muted) {
        client.mute = data.config.volume.muted;
      }
    }
  }

  if (client.name === undefined && data.host && data.host.name) {
    client.name = data.host.name;
  }

  return client;
};

/**
 * Format a snapcast client object into a universal format
 *
 * @param data obj
 * @return obj
 * */
export const formatGroup = function (data) {
  const group = {};
  const fields = [
    'id',
    'name',
    'mute',
    'stream_id',
    'clients_ids',
  ];

  for (const field of fields) {
    if (data.hasOwnProperty(field)) {
      group[field] = data[field];
    }
  }

  if (group.name === undefined || group.name === '') {
    group.name = `Group ${data.id.substring(0, 3)}`;
  }

  if (group.mute === undefined && data.mute !== undefined) {
    group.mute = data.mute;
  }

  return group;
};


/**
 * Collate an object with external references into a fully self-contained object
 * We merge *_uris references (ie tracks_uris) into the main object
 *
 * @param object Obj
 * @param indexes Obj (the relevant core indexes)
 * @return object Obj
 * */
export const collate = function (obj, indexes = {}) {
  // First, let's reset this object
  // This is important because by changing this object, we inadvertently
  // change the source object (ie the indexed record), which undoes the
  // efficiencies of a lean index object
  obj = { ...obj };

  // Setup empty arrays for the appropriate reference objects
  // This helps create a consistent object structure
  if (obj.artists_uris !== undefined) 		obj.artists = [];
  if (obj.albums_uris !== undefined) 			obj.albums = [];
  if (obj.tracks_uris !== undefined) 			obj.tracks = [];
  if (obj.users_uris !== undefined) 			obj.users = [];
  if (obj.playlists_uris !== undefined) 		obj.playlists = [];
  if (obj.related_artists_uris !== undefined) obj.related_artists = [];
  if (obj.clients_ids !== undefined) 			obj.clients = [];

  if (indexes.artists) {
    if (obj.artists_uris) {
      for (var uri of obj.artists_uris) {
        if (indexes.artists[uri]) {
          obj.artists.push(indexes.artists[uri]);
        }
      }
    }
    if (obj.related_artists_uris) {
      for (var uri of obj.related_artists_uris) {
        if (indexes.artists[uri]) {
          obj.related_artists.push(indexes.artists[uri]);
        }
      }
    }
    if (obj.artist_uri) {
      if (indexes.artists[obj.artist_uri]) {
        obj.artist = indexes.artists[obj.artist_uri];
      }
    }
  }

  if (indexes.albums) {
    if (obj.albums_uris) {
      for (var uri of obj.albums_uris) {
        if (indexes.albums[uri]) {
          obj.albums.push(indexes.albums[uri]);
        }
      }
    }
    if (obj.album_uri) {
      if (indexes.albums[obj.album_uri]) {
        obj.album = indexes.albums[obj.album_uri];
      }
    }
  }

  if (indexes.tracks) {
    if (obj.tracks_uris) {
      for (var uri of obj.tracks_uris) {
        if (indexes.tracks[uri]) {
          obj.tracks.push(indexes.tracks[uri]);
        }
      }
    }
    if (obj.track_uri) {
      if (indexes.tracks[obj.track_uri]) {
        obj.track = indexes.tracks[obj.track_uri];
      }
    }
  }

  if (indexes.users) {
    if (obj.users_uris) {
      for (var uri of obj.users_uris) {
        if (indexes.users[uri]) {
          obj.users.push(indexes.users[uri]);
        }
      }
    }
    if (obj.user_uri) {
      if (indexes.users[obj.user_uri]) {
        obj.user = indexes.users[obj.user_uri];
      }
    }
  }

  if (indexes.playlists) {
    if (obj.playlists_uris) {
      for (var uri of obj.playlists_uris) {
        if (indexes.playlists[uri]) {
          obj.playlists.push(indexes.playlists[uri]);
        }
      }
    }
    if (obj.playlist_uri) {
      if (indexes.playlists[obj.playlist_uri]) {
        obj.playlist = indexes.playlists[obj.playlist_uri];
      }
    }
  }

  if (indexes.clients) {
    if (obj.clients_ids) {
      for (const id of obj.clients_ids) {
        if (indexes.clients[id]) {
          obj.clients.push(indexes.clients[id]);
        }
      }
    }
  }

  return obj;
};


/**
 * Figure out a URI's source namespace
 * @param uri = string
 * */
export let uriSource = function (uri) {
  if (!uri) {
    return false;
  }
  const exploded = uri.split(':');
  return exploded[0];
};
/**
 * Identify what kind of asset a URI is (playlist, album, etc)
 *
 * @param uri = string
 * @return string
 * */
export const uriType = function (uri) {
  if (!uri) return null;

  const exploded = uri.split(':');

  if (exploded[0] == 'm3u') {
    return 'playlist';
  }

  if (exploded[0] == 'iris') {
    	switch (exploded[1]) {
	    	case 'search':
	    	case 'discover':
	    	case 'browse':
        case 'radio':
	    		return exploded[1];
	    		break;
	    }
  }

  switch (exploded[1]) {
    	case 'track':
    	case 'artist':
    	case 'album':
    	case 'playlist':
    	case 'genre':
    		return exploded[1];
    		break;

    	case 'user':
    		if (exploded.length > 3 && exploded[3] == 'playlist') {
    			return 'playlist';
    		}
    		return exploded[1];
    		break;
  }
};


export const sourceIcon = function (uri, source = null) {
  if (uri) source = uriSource(uri);
  switch (source) {
    case 'local':
    case 'm3u':
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
      return 'microphone';

    default:
      return source;
  }
};


/**
 * Get an element from a URI
 * @param element = string, the element we wish to extract
 * @param uri = string
 * */
export const getFromUri = function (element, uri = '') {
  const exploded = uri.split(':');
  const namespace = exploded[0];

  switch (element) {
    	case 'mbid':
	        var index = exploded.indexOf('mbid');
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
    		} if (exploded[1] == 'user' && exploded[3] == 'playlist') {
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

    	case 'seeds':
    		if (exploded[1] == 'discover') {
    			return exploded[2];
    		}
    		break;

    	case 'searchtype':
    		if (exploded[0] == 'search') {
        return exploded[1];
    		}
    		break;

    	case 'searchterm':
    		if (exploded[0] == 'search') {
        return exploded[2];
    		}
    		break;
  }
  return null;
};


/**
 * Build a link to an asset. Using the URI type we can ascertain where we need
 * to direct the user (eg /track/local:track:1235.mp3)
 *
 * @param $uri = String
 * @return String
 * */
export const buildLink = function (uri) {
  // Start the link with the URI type
  const type = uriType(uri);
  let link = `/${type}/`;

  // Encode the whole URI as though it's a component. This makes it URL friendly for
  // all Mopidy backends (some use URIs like local:track:http://rss.com/stuff.mp3) which
  // is never going to work nicely.
  uri = encodeURIComponent(uri);
  link += uri;

  return link;
};


/**
 * Digest an array of objects and pull into simple array of one property
 *
 * @param property = string
 * @param items = Array
 * @return Array
 * */
export const arrayOf = function (property, items) {
  const array = [];
  for (const item of items) {
    // Make sure the property is defined
    if (item[property] !== undefined && item[property] != null) {
      array.push(item[property]);
    }
  }
  return array;
};


/**
 * Merge duplicated items in an array
 *
 * @param list Array the unclean array
 * @param key string = the unique key (id, uri, tlid, etc)
 * */
export const mergeDuplicates = function (list, key) {
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
export const removeDuplicates = function (array) {
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
export const applyFilter = function (field, value, array, singular = false) {
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
export const createRange = function (indexes) {
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
export const sortItems = function (array, property, reverse = false, sort_map = null) {
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
    if (property == 'uri') {
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
      return 0;

      // Numeric sorting
    } else if (typeof a_value === 'number' && typeof b_value === 'number') {
      if (a_value == null && b_value == null) return 0;
      if (a_value == null) return -1;
      if (b_value == null) return 1;
      if (parseInt(a_value) > parseInt(b_value)) return 1;
      if (parseInt(a_value) < parseInt(b_value)) return -1;
      return 0;

      // Alphabetic sorting
    } else {
      if (a_value && !b_value) return -1;
      if (!a_value && b_value) return 1;
      if (!a_value && !b_value) return 0;
      if (a_value.toLowerCase() > b_value.toLowerCase()) return 1;
      if (a_value.toLowerCase() < b_value.toLowerCase()) return -1;
      return 0;
    }
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
export const shuffle = function (array) {
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

/**
 * Figure out if a value is a number
 * @param value = mixed
 * @return boolean
 * */
export const isNumeric = function (value) {
  return !isNaN(parseFloat(value)) && isFinite(value);
};


/**
 * Figure out if a value is an object
 * @param value = mixed
 * @return boolean
 * */
export let isObject = function (value) {
  return value instanceof Object && value.constructor === Object;
};


/**
 * Detect if an item is in the loading queue. We simply loop all load items to
 * see if any items contain our searched key.
 *
 * TODO: Explore performance of this
 * TODO: Allow wildcards
 *
 * @param load_queue = obj (passed from store)
 * @param key = string (the string to lookup)
 * @return boolean
 * */
export const isLoading = function (load_queue = [], keys = []) {
  // Loop all of our load queue items
  for (const load_queue_key in load_queue) {
    // Make sure it's not a root object method
    if (load_queue.hasOwnProperty(load_queue_key)) {
      // Loop all the keys we're looking for
      for (let i = 0; i < keys.length; i++) {
        if (load_queue[load_queue_key].includes(keys[i])) {
          return true;
        }
      }
    }
  }
  return false;
};


/**
 * Is this app running from the hosted instance?
 * For example the GitHub-hosted UI
 *
 * @param Array hosts = valid hosted domain names
 * @return Boolean
 * */
export const isHosted = function (hosts = ['jaedb.github.io']) {
  const { hostname } = window.location;
  return hosts.includes(hostname);
};


/**
 * Get indexed record(s) by URI from our asset index
 *
 * @param store = obj
 * @param uris = mixed (array or string)
 * @return array
 * */
export const getIndexedRecords = function (index, uris) {
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
export const titleCase = function (string) {
  return string.charAt(0).toUpperCase() + string.slice(1);
};


/**
 * Scroll to the top of the page
 * Our 'content' is housed in the <main> DOM element
 * We make sure the target supports scrolling before we attempt it
 * Safari and IE Edge, well, don't.
 *
 * @param target String (element ID, optional)
 * @param smooth_scroll Boolean (optional)
 * */
export const scrollTo = function (target = null, smooth_scroll = false) {
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
export const upgradeSpotifyPlaylistUris = function (uris) {
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
export let upgradeSpotifyPlaylistUri = function (uri) {
  return upgradeSpotifyPlaylistUris([uri])[0];
};

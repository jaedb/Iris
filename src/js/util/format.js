import { compact } from 'lodash';
import {
  isObject,
  upgradeSpotifyPlaylistUri,
  uriSource,
  uriType,
  getFromUri,
} from './helpers';

/**
 * Convert a string to JSON, after we've checked whether it needs
 * conversion or not.
 *
 * @param data String or Object
 * @return Object
 * */
const toJSON = (data) => {
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
 * Get a track's icon
 * @param track object
 * @return string
 * */
const getTrackIcon = function (current_track = false, core = false) {
  if (!core) return false;
  if (!current_track) return false;
  if (typeof (current_track.uri) === 'undefined') return false;
  if (typeof (core.items[current_track.uri]) === 'undefined') return false;
  const track = core.items[current_track.uri];
  if (!track.images) return false;
  return formatImages(track.images).small;
};

/**
 * Digest an array of Mopidy image objects into a universal format. We also re-write
 * image URLs to be absolute to the mopidy server (required for proxy setups).
 *
 * @param mopidy = obj (mopidy store object)
 * @param images = array
 * @return array
 * */
const digestMopidyImages = function (mopidy, images) {
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


/**
 * Format image URLs into a consistent size-based object
 * We digest all our known image source formats into a universal small,medium,large,huge object
 *
 * @param $data mixed
 * @return Object
 * */
const formatImages = function (data) {
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
const formatSimpleObject = function (data) {
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
const formatTracks = function (records = []) {
  const formatted = [];
  for (const record of records) {
	    formatted.push(formatTrack(record));
  }
  return formatted;
};
const formatAlbums = function (records = []) {
  const formatted = [];
  for (const record of records) {
	    formatted.push(formatAlbum(record));
  }
  return formatted;
};
const formatArtists = function (records = []) {
  const formatted = [];
  for (const record of records) {
	    formatted.push(formatArtist(record));
  }
  return formatted;
};
const formatPlaylists = function (records = []) {
  const formatted = [];
  for (const record of records) {
	    formatted.push(formatPlaylist(record));
  }
  return formatted;
};
const formatUsers = function (records = []) {
  const formatted = [];
  for (const record of records) {
	    formatted.push(formatUser(record));
  }
  return formatted;
};
const formatCategories = function (records = []) {
  const formatted = [];
  for (const record of records) {
	    formatted.push(formatCategory(record));
  }
  return formatted;
};
const formatSimpleObjects = function (records = []) {
  const formatted = [];
  for (const record of records) {
	    formatted.push(formatSimpleObject(record));
  }
  return formatted;
};

/**
 * Prepare a URI for use in a URL
 *
 * Simple alias to encodeURIComponent so this can be extended as needed
 * @param {String} uri
 */
const encodeUri = (rawUri = '') => {
  let uri = rawUri;

  // Manually encode %
  // Needed because React Router History incorrectly handles '%' during encoding/decoding
  // See https://github.com/jaedb/Iris/issues/674
  uri = uri.replace(/%25/g, '_PRCNT_');
  uri = encodeURIComponent(uri);
  return uri;
};

/**
 * Rebuild a URI with some ugly-ass handling of encoding.
 *
 * We can't just run encodeURIComponent because it will encode ':' and '/' elements which are
 * required to properly map to Mopidy's internal structure. Some things need encoding, and others
 * need decoding *sigh*.
 *
 * Instead, we pluck the individual encodings as needed.
 *
 * For example somebackend:track:https://youtube.com/1234 would cause issues with the
 * https:// section. Our explode-by-':' approach would break this type of URI.
 *
 * @param {String} rawUri
 */
const decodeUri = (rawUri = '') => {
  let uri = rawUri;

  try {
    uri = decodeURIComponent(uri);
  } catch {
    console.error('Could not decode URI', uri);
  }

  // Some characters must be encoded for Mopidy URI compatibility
  uri = uri.replace(/%2F/g, '/'); // We need slashes
  uri = uri.replace(/_PRCNT_/g, '%25'); // Decode '%'
  uri = uri.replace(/!/g, '%21');
  uri = uri.replace(/\*/g, '%2A');
  uri = uri.replace(/\(/g, '%28');
  uri = uri.replace(/\)/g, '%29');
  uri = uri.replace(/\[/g, '%5B');
  uri = uri.replace(/\]/g, '%5D');
  uri = uri.replace(/@/g, '%40');
  uri = uri.replace(/#/g, '%23');
  uri = uri.replace(/\$/g, '%24');
  uri = uri.replace(/&/g, '%26');
  uri = uri.replace(/'/g, '%27');
  uri = uri.replace(/,/g, '%2C');
  uri = uri.replace(/ /g, '%20');

  return uri;
};

const encodeMopidyUri = (rawUri = '') => {
  let uri = rawUri;

  return uri;
}

/**
 * Format our album objects into a universal format
 *
 * @param data obj
 * @return obj
 * */
const formatAlbum = function (data) {
  const album = { type: 'album' };
  const fields = [
    'uri',
    'in_library',
    'provider',
    'name',
    'album_type',
    'last_modified',
    'added_at',
    'release_date',
    'listeners',
    'play_count',
    'wiki',
    'wiki_publish_date',
    'popularity',
    'images',
    'tracks',
    'tracks_total',
    'artists',
  ];

  // Nested album object (eg in spotify library)
  if (data && data.album && isObject(data.album)) {
    if (data.added_at) {
      data.album.added_at = data.added_at;
    }
    data = data.album;
  }

  // Loop fields and import from data
  for (const field of fields) {
    if (data.hasOwnProperty(field)) {
      album[field] = data[field];
    }
  }

  if (album.images && !album.images.formatted) {
    album.images = formatImages(album.images);
  }
  if (album.tracks) {
    if (album.tracks.items) {
      album.tracks = formatTracks(album.tracks.items);
    } else {
      album.tracks = formatTracks(album.tracks);
    }
    album.tracks_total = album.tracks.length;
  }
  if (album.artists) {
    album.artists = formatSimpleObjects(album.artists);
  }

  if (data.last_modified && album.added_at === undefined) {
    album.added_at = data.last_modified;
  } else if (data.added_at && album.last_modified === undefined) {
    album.last_modified = data.added_at;
  }
  if (data.date && !album.date) {
    album.release_date = data.date;
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
const formatArtist = function (data) {
  const artist = { type: 'artist' };
  const fields = [
    'uri',
    'in_library',
    'provider',
    'mbid',
    'name',
    'popularity',
    'followers',
    'listeners',
    'added_at',
    'biography',
    'biography_link',
    'biography_publish_date',
    'related_artists',
    'albums_uris',
    'tracks',
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
const formatPlaylist = function (data) {
  const playlist = { type: 'playlist' };
  const fields = [
    'uri',
    'in_library',
    'snapshot_id',
    'provider',
    'collaborative',
    'public',
    'name',
    'description',
    'images',
    'popularity',
    'followers',
    'last_modified',
    'can_edit',
    'user',
    'tracks',
    'tracks_total',
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
  if (playlist.tracks) {
    if (Array.isArray(playlist.tracks)) {
      playlist.tracks = formatTracks(playlist.tracks);
    } else if (playlist.tracks.items) {
      playlist.tracks = formatTracks(playlist.tracks.items);
    } else {
      playlist.tracks = null;
    }
  }

  if (playlist.tracks_total === undefined) {
    playlist.tracks_total = playlist.tracks ? playlist.tracks.length : 0;
  }

  if (data.last_modified_date && playlist.last_modified === undefined) {
    playlist.last_modified = data.last_modified_date;
  }

  if (data.followers && data.followers.total !== undefined) {
    playlist.followers = data.followers.total;
  }

  if (data.tracks && data.tracks.total !== undefined) {
    playlist.tracks_total = data.tracks.total;
  }

  if (playlist.last_modified && playlist.added_at === undefined) {
    playlist.added_at = data.last_modified;
  } else if (playlist.added_at && playlist.last_modified === undefined) {
    playlist.last_modified = data.added_at;
  }

  if (data.owner && playlist.user === undefined) {
    playlist.user = {
      id: data.owner.id,
      uri: data.owner.uri,
      name: (data.owner.display_name || data.owner.id),
    };
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
const formatUser = function (data) {
  const user = { type: 'user' };
  const fields = [
    'id',
    'in_library',
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
const formatTrack = function (data) {
  const track = { type: 'track' };
  const fields = [
    'loading',
    'uri',
    'in_library',
    'tlid',
    'provider',
    'name',
    'images',
    'release_date',
    'disc_number',
    'track_number',
    'disc_track',
    'duration',
    'followers',
    'popularity',
    'userloved',
    'last_modified',
    'added_at',
    'is_explicit',
    'is_local',
    'lyrics',
    'lyrics_path',
    'lyrics_results',
    'artists', // Array of simple records
    'album', // Array of simple records
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

  if (track.last_modified && track.added_at === undefined) {
    track.added_at = track.last_modified;
  } else if (track.added_at && track.last_modified === undefined) {
    track.last_modified = track.added_at;
  }

  if (data.duration_ms !== undefined) {
    track.duration = data.duration_ms;
  } else if (data.length !== undefined) {
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

  if (track.artists) {
    track.artists = formatSimpleObjects(track.artists);
  }

  if (track.album) {
    track.album = formatSimpleObject(track.album);
  }

  // Produce a sort-friendly disc_track float (XXX.XXX)
  track.disc_track = `${track.disc_number || 0}`.padStart(3, '0');
  track.disc_track += '.';
  track.disc_track += `${track.track_number || 0}`.padStart(3, '0');

  // Remove lower-case encoding of ':'
  // See https://github.com/tkem/mopidy-dleyna/issues/72
  track.uri = decodeUri(track.uri);

  return track;
};

/**
 * Format a snapcast client object into a universal format
 *
 * @param data obj
 * @return obj
 * */
const formatClient = function (data) {
  const client = { type: 'client' };
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

    if (data.config.volume !== undefined) {
      if (data.config.volume.percent !== undefined) {
        client.volume = data.config.volume.percent;
      }
      if (data.config.volume.muted) {
        client.mute = data.config.volume.muted;
      }
    }
  } else {
    if (client.latency === undefined && data.latency !== undefined) {
      client.latency = data.latency;
    }
    if (typeof data.volume === 'object') {
      if (data.volume.percent) {
        client.volume = data.volume.percent;
      }
      if (data.volume.muted !== undefined) {
        client.mute = data.volume.muted;
      }
    }
  }

  if (client.name === undefined && data.host && data.host.name) {
    client.name = data.host.name;
  }

  return client;
};

/**
 * Spotify playlists category
 *
 * @param data obj
 * @return obj
 * */
const formatCategory = function (data) {
  const category = { type: 'category' };
  const fields = [
    'id',
    'uri',
    'name',
    'playlists_uris',
  ];

  for (const field of fields) {
    if (data.hasOwnProperty(field)) {
      category[field] = data[field];
    }
  }

  if (!category.uri && data.id) {
    category.uri = `spotify:category:${data.id}`;
  }
  
  if (data.icons) {
    category.images = formatImages(data.icons);
  }

  return category;
};

/**
 * Format a snapcast client object into a universal format
 *
 * @param data obj
 * @return obj
 * */
const formatGroup = function (data) {
  const group = { type: 'group' };
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

  if (group.mute === undefined && data.muted !== undefined) {
    group.mute = data.muted;
  }

  return group;
};


const collateLibrary = (uris, itemsIndex) => {
  return compact(uris.map((uri) => itemsIndex[uri]));
};

/**
 * Collate an object with external references into a fully self-contained object
 * We merge *_uris references (ie tracks_uris) into the main object
 *
 * @param object Obj
 * @param indexes Obj (the relevant core indexes)
 * @return object Obj
 * */
const collate = function (obj, indexes = {}) {
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
  if (obj.items_uris !== undefined) 			obj.items = [];

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

  if (indexes.items) {
    if (obj.items_uris) {
      for (const uri of obj.items_uris) {
        if (indexes.items[uri]) {
          obj.items.push(indexes.items[uri]);
        }
      }
    }
  }

  return obj;
};

/**
 * Add the array index as our sort_id.
 * This allows us a source of truth for the original order of items when we have applied
 * sort rules in the UI.
 *
 * @param {Array} array of objects to have sort_id added
 */
const injectSortId = (array) => {
  return array.map((item, index) => ({ ...item, sort_id: index }));
};

export {
  toJSON,
  getTrackIcon,
  digestMopidyImages,
  formatImages,
  formatSimpleObject,
  formatSimpleObjects,
  formatAlbum,
  formatAlbums,
  formatArtist,
  formatArtists,
  formatPlaylist,
  formatPlaylists,
  formatUser,
  formatUsers,
  formatTrack,
  formatTracks,
  formatClient,
  formatGroup,
  formatCategory,
  formatCategories,
  collate,
  collateLibrary,
  injectSortId,
  encodeUri,
  decodeUri,
  encodeMopidyUri,
};

export default {
  toJSON,
  getTrackIcon,
  digestMopidyImages,
  formatImages,
  formatSimpleObject,
  formatSimpleObjects,
  formatAlbum,
  formatAlbums,
  formatArtist,
  formatArtists,
  formatPlaylist,
  formatPlaylists,
  formatUser,
  formatUsers,
  formatTrack,
  formatTracks,
  formatClient,
  formatGroup,
  formatCategory,
  formatCategories,
  collate,
  collateLibrary,
  injectSortId,
  encodeUri,
  decodeUri,
  encodeMopidyUri,
};


import { removeDuplicates, arrayOf } from '../../util/arrays';
import { formatTracks } from '../../util/format';

export default function reducer(core = {}, action) {
  switch (action.type) {
    case 'CORE_SET':
      return { ...core, ...action.data };

    case 'CACHEBUST_HTTP_STREAM':
      return { ...core, http_streaming_cachebuster: new Date().getTime() };

      /**
         * Current track and tracklist
         * */

    case 'CURRENT_TRACK_LOADED':
      return {
        ...core,
        current_track: action.track,
        current_track_uri: action.uri,
      };

    case 'CLEAR_CURRENT_TRACK':
      return {
        ...core,
        current_track: null,
        current_track_uri: null,
      };

    case 'NEXT_TRACK_LOADED':
      return { ...core, next_track_uri: action.uri };

    case 'QUEUE_LOADED':
      return { ...core, queue: action.tracks };

    case 'PUSHER_QUEUE_METADATA':
    case 'PUSHER_QUEUE_METADATA_CHANGED':
      var tracklist = Object.assign([], core.current_tracklist);
      for (let i = 0; i < tracklist.length; i++) {
        // load our metadata (if we have any for that tlid)
        if (action.queue_metadata[`tlid_${tracklist[i].tlid}`] !== undefined) {
          tracklist[i] = {

            ...tracklist[i],
            ...action.queue_metadata[`tlid_${tracklist[i].tlid}`],
          };
        }
      }
      return { ...core, current_tracklist: tracklist, queue_metadata: action.queue_metadata };

    case 'PUSHER_RADIO_LOADED':
    case 'PUSHER_RADIO_STARTED':
    case 'PUSHER_RADIO_CHANGED':
    case 'PUSHER_RADIO_STOPPED':
      return { ...core, seeds_resolved: false, radio: action.radio };

    case 'RADIO_SEEDS_RESOLVED':
      var radio = { ...core.radio, resolved_seeds: action.resolved_seeds };
      return { ...core, radio };


      /**
         * Index updates
         * These actions are only ever called by middleware after we've digested one more many assets
         * and appended to their relevant index.
         * */

    case 'TRACKS_LOADED':
      var tracks = { ...core.tracks };
      for (const track of action.tracks) {
        tracks[track.uri] = track;
      }
      return { ...core, tracks };

    case 'ALBUMS_LOADED':
      var albums = { ...core.albums };
      for (const album of action.albums) {
        albums[album.uri] = album;
      }
      return { ...core, albums };

    case 'ARTISTS_LOADED':
      var artists = { ...core.artists };
      for (var artist of action.artists) {
        artists[artist.uri] = artist;
      }
      return { ...core, artists };

    case 'PLAYLISTS_LOADED':
      var playlists = { ...core.playlists };
      for (var playlist of action.playlists) {
        playlists[playlist.uri] = playlist;
      }
      return { ...core, playlists };

    case 'USERS_LOADED':
      var users = { ...core.users };
      for (var user of action.users) {
        users[user.uri] = user;
      }
      return { ...core, users };


    case 'ARTIST_ALBUMS_LOADED':
      var artists = { ...core.artists };
      var albums_uris = [];
      if (artists[action.artist_uri].albums_uris) {
        albums_uris = artists[action.artist_uri].albums_uris;
      }

      var artist = {

        ...artists[action.artist_uri],
        albums_uris: [...albums_uris, ...action.albums_uris],
        albums_more: action.more,
        albums_total: action.total,
      };
      artists[action.artist_uri] = artist;
      return { ...core, artists };


    case 'USER_PLAYLISTS_LOADED':
      var users = { ...core.users };
      var existing_playlists_uris = [];
      if (users[action.uri] && users[action.uri].playlists_uris) {
            	existing_playlists_uris = users[action.uri].playlists_uris;
      }

      var playlists_uris = [...existing_playlists_uris, ...arrayOf('uri', action.playlists)];

      var user = {

        ...users[action.uri],
        playlists_uris,
        playlists_more: action.more,
        playlists_total: action.total,
      };
      users[action.uri] = user;
      return { ...core, users };


      /**
         * Remove an item from an index
         * */

    case 'REMOVE_FROM_INDEX':
      var index = { ...core[action.index_name] };

      // We have a new key to redirect to
      if (action.new_key) {
            	index[action.key] = {
            		moved_to: action.new_key,
            	};

        // No redirection, so just a clean delete
      } else {
            	delete index[action.key];
      }

      var updated_core = {};
      updated_core[action.index_name] = index;

      return { ...core, ...updated_core };


      /**
         * Playlists
         * */

    case 'PLAYLIST_TRACKS':
      var playlists = { ...core.playlists };
      var playlist = { ...playlists[action.key], tracks_uris: action.tracks_uris };

      playlists[action.key] = playlist;
      return { ...core, playlists };

    case 'LIBRARY_PLAYLISTS_LOADED':
      if (core.library_playlists) {
        var library_playlists = [...core.library_playlists, ...action.uris];
      } else {
        var library_playlists = action.uris;
      }

      library_playlists = removeDuplicates(library_playlists);

      return {
        ...core,
        library_playlists,
        library_playlists_started: true,
      };


      /**
         * Genres
         * */

    case 'SPOTIFY_GENRES_LOADED':
      return { ...core, genres: action.genres };

    case 'VIEW_DATA_LOADED':
      return {
        ...core,
        view: {
          ...(core.view ? core.view : {}),
          ...action.data,
        },
      };


      /**
         * Search results
         * */

    case 'SEARCH_STARTED':
      return {
        ...core,
        search_results: {
          artists_uris: [],
          albums_uris: [],
          playlists_uris: [],
          tracks: [],
        },
      };

    case 'SEARCH_RESULTS_LOADED':

      // artists
      if (core.search_results && core.search_results.artists_uris) {
        var { artists_uris } = core.search_results;
      } else {
        var artists_uris = [];
      }
      if (action.artists_uris) artists_uris = [...artists_uris, ...action.artists_uris];

      // more tracks
      if (typeof (action.artists_more) !== 'undefined') var { artists_more } = action;
      else if (core.search_results && core.search_results.artists_more) var { artists_more } = core.search_results;
      else var artists_more = null;


      // albums
      if (core.search_results && core.search_results.albums_uris) {
        var { albums_uris } = core.search_results;
      } else {
        var albums_uris = [];
      }
      if (action.albums_uris) albums_uris = [...albums_uris, ...action.albums_uris];

      // more tracks
      if (typeof (action.albums_more) !== 'undefined') var { albums_more } = action;
      else if (core.search_results && core.search_results.albums_more) var { albums_more } = core.search_results;
      else var albums_more = null;


      // playlists
      if (core.search_results && core.search_results.playlists_uris) {
        var { playlists_uris } = core.search_results;
      } else {
        var playlists_uris = [];
      }
      if (action.playlists_uris) playlists_uris = [...playlists_uris, ...action.playlists_uris];

      // more tracks
      if (typeof (action.playlists_more) !== 'undefined') var { playlists_more } = action;
      else if (core.search_results && core.search_results.playlists_more) var { playlists_more } = core.search_results;
      else var playlists_more = null;


      // tracks
      if (core.search_results && core.search_results.tracks) {
        var { tracks } = core.search_results;
      } else {
        var tracks = [];
      }
      if (action.tracks) tracks = [...tracks, ...formatTracks(action.tracks)];

      // more tracks
      if (typeof (action.tracks_more) !== 'undefined') var { tracks_more } = action;
      else if (core.search_results && core.search_results.tracks_more) var { tracks_more } = core.search_results;
      else var tracks_more = null;

      return {
        ...core,
        search_results: {
          artists_more,
          artists_uris: removeDuplicates(artists_uris),
          albums_more,
          albums_uris: removeDuplicates(albums_uris),
          playlists_more,
          playlists_uris: removeDuplicates(playlists_uris),
          tracks,
          tracks_more,
        },
      };


    default:
      return core;
  }
}

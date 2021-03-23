import { arrayOf } from '../../util/arrays';

export default function reducer(core = {}, action) {
  switch (action.type) {
    case 'CORE_SET':
      return { ...core, ...action.data };

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
        stream_title: null,
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

    case 'STREAM_TITLE_CHANGED':
    case 'STREAM_TITLE_LOADED':
      return { ...core, stream_title: action.stream_title };

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

    case 'ITEM_LOADED':
      return {
        ...core,
        items: {
          ...core.items,
          [action.item.uri]: action.item,
        },
      };

    case 'ITEMS_LOADED':
      const mergedItems = action.items.reduce(
        (obj, item) => (obj[item.uri] = item, obj),
        {},
      );
      return {
        ...core,
        items: {
          ...core.items,
          ...mergedItems,
        },
      };

    case 'LIBRARY_LOADED':
      return {
        ...core,
        libraries: {
          ...core.libraries,
          [action.library.uri]: action.library,
        },
      };

    case 'UNLOAD_LIBRARY': {
      const libraries = { ...core.libraries };
      delete libraries[action.uri];
      return {
        ...core,
        libraries,
      };
    }

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

    case 'RESTORE_LIBRARY_FROM_COLD_STORE':
      const libraries = { ...core.libraries };
      libraries[action.library.uri] = {
        ...(libraries[action.library.uri] || {}),
        ...action.library,
      };
      return {
        ...core,
        libraries,
      };

    case 'RESTORE_ITEMS_FROM_COLD_STORE': {
      const items = { ...core.items };
      action.items.forEach((item) => {
        items[item.uri] = {
          ...(items[item.uri] || {}),
          ...item,
        };
      });
      return {
        ...core,
        items,
      };
    }

    case 'REMOVE_ITEM': {
      const items = { ...core.items };
      if (action.new_key) {
        items[action.key] = { moved_to: action.new_key };
      } else {
        delete items[action.key];
      }

      return { ...core, items };
    }

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
    case 'START_SEARCH':
      return {
        ...core,
        search_results: {
          query: action.query,
          artists: [],
          albums: [],
          playlists: [],
          tracks: [],
        },
      };

    case 'SEARCH_RESULTS_LOADED': {
      const { search_results } = action;
      return {
        ...core,
        search_results,
      };
    }

    default:
      return core;
  }
}

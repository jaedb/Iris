
import { removeDuplicates } from '../../util/arrays';

export default function reducer(mopidy = {}, action) {
  switch (action.type) {
    case 'MOPIDY_SET':
      return { ...mopidy, ...action.data };

    case 'MOPIDY_UPDATE_SERVERS':
      return { ...mopidy, servers: action.servers };

    case 'MOPIDY_CONNECT':
    case 'MOPIDY_CONNECTING':
      return {
        ...mopidy,
        connected: false,
        connecting: true,
      };

    case 'MOPIDY_CONNECTED':
      return {
        ...mopidy,
        connected: true,
            	connecting: false,
            	restart_running: false,
            	upgrade_running: false,
      };

    case 'MOPIDY_DISCONNECTED':
      return {
        ...mopidy,
        connected: false,
            	connecting: false,
      };

    case 'MOPIDY_CHANGE_TRACK':
      return { ...mopidy, tlid: action.tlid };

    case 'MOPIDY_URI_SCHEMES':
      return { ...mopidy, uri_schemes: action.uri_schemes };

    case 'MOPIDY_RESTART_STARTED':
      return { ...mopidy, restart_running: true };

    case 'MOPIDY_RESTART_FINISHED':
      return { ...mopidy, restart_running: false };

    case 'MOPIDY_UPGRADE_STARTED':
      return { ...mopidy, upgrade_running: true };

    case 'MOPIDY_UPGRADE_FINISHED':
      return { ...mopidy, upgrade_running: false };

    case 'MOPIDY_LOCAL_SCAN_STARTED':
      return { ...mopidy, local_scan_running: true };

    case 'MOPIDY_LOCAL_SCAN_FINISHED':
      return { ...mopidy, local_scan_running: false };


      /**
         * State-oriented actions
         * */
    case 'MOPIDY_PLAY_STATE':
      return { ...mopidy, play_state: action.play_state };

    case 'MOPIDY_CONSUME':
      return { ...mopidy, consume: action.consume };

    case 'MOPIDY_RANDOM':
      return { ...mopidy, random: action.random };

    case 'MOPIDY_REPEAT':
      return { ...mopidy, repeat: action.repeat };

    case 'MOPIDY_VOLUME':
      return { ...mopidy, volume: action.volume };

    case 'MOPIDY_MUTE':
      return { ...mopidy, mute: action.mute };

    case 'MOPIDY_TIME_POSITION':
      return { ...mopidy, time_position: action.time_position };

    case 'MOPIDY_QUEUE_HISTORY':
      var history = [];
      for (let i = 0; i < action.tracks.length; i++) {
        history.push({

          ...action.tracks[i][1],
          played_at: action.tracks[i][0],
          type: 'history',
        });
      }
      return { ...mopidy, queue_history: history };


      /**
         * Directories
         * This also facilitates all backend-only music providers (SoundCloud, Dirble, etc)
         * */

    case 'MOPIDY_DIRECTORY_FLUSH':
      return { ...mopidy, directory: null };

    case 'MOPIDY_DIRECTORY_LOADED':
      return {
        ...mopidy,
        directory: { ...mopidy.directory, ...action.directory },
      };


      /**
         * Library
         * */

    case 'MOPIDY_LIBRARY_PLAYLISTS_LOADED':
      if (mopidy.library_playlists) {
        var uris = [...mopidy.library_playlists, ...action.uris];
      } else {
        var { uris } = action;
      }
      return { ...mopidy, library_playlists: removeDuplicates(uris) };

    case 'MOPIDY_LIBRARY_PLAYLISTS_LOADED_ALL':
      return { ...mopidy, library_playlists_loaded_all: true };

    case 'MOPIDY_LIBRARY_PLAYLIST_CREATED':
      var library_playlists = [];
      if (mopidy.library_playlists) {
        library_playlists = Object.assign([], mopidy.library_playlists);
      }
      library_playlists.push(action.key);
      return { ...mopidy, library_playlists };

    case 'MOPIDY_LIBRARY_PLAYLIST_DELETED':
      var library_playlists = [];
      if (mopidy.library_playlists) {
        library_playlists = Object.assign([], mopidy.library_playlists);
        library_playlists.splice(library_playlists.indexOf(action.uri), 1);
      }
      return { ...mopidy, library_playlists };

    case 'MOPIDY_LIBRARY_ARTISTS_LOADED':
      if (mopidy.library_artists) {
        var uris = [...mopidy.library_artists, ...action.uris];
      } else {
        var { uris } = action;
      }
      return { ...mopidy, library_artists: removeDuplicates(uris) };

    case 'MOPIDY_CLEAR_LIBRARY_ARTISTS':
      return { ...mopidy, library_artists: null };

    case 'MOPIDY_LIBRARY_ALBUMS_LOADED':
      if (mopidy.library_albums) {
        var uris = [...mopidy.library_albums, ...action.uris];
      } else {
        var { uris } = action;
      }
      return { ...mopidy, library_albums: removeDuplicates(uris) };

    case 'MOPIDY_CLEAR_LIBRARY_ALBUMS':
      return { ...mopidy, library_albums: null };


      /**
         * Searching
         * */

    case 'MOPIDY_CLEAR_SEARCH_RESULTS':
      return { ...mopidy, search_results: {} };

    case 'MOPIDY_SEARCH_RESULTS_LOADED':

      // Fetch or create our container
      if (mopidy.search_results) {
        var search_results = { ...mopidy.search_results };
      } else {
        var search_results = {};
      }

      search_results.query = action.query;

      if (search_results[action.context]) {
        search_results[action.context] = [...search_results[action.context], ...action.results];
      } else {
        search_results[action.context] = action.results;
      }

      return { ...mopidy, search_results };

    default:
      return mopidy;
  }
}

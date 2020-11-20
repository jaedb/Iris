
import storage from '../../util/storage';

const localstorageMiddleware = (function () {
  return (store) => (next) => (action) => {
    // proceed as normal first
    // this way, any reducers and middleware do their thing BEFORE we store our new state
    next(action);

    // append our state to a global variable. This gives us access to debug the store at any point
    window._store = store;

    // if debug enabled
    if (store.getState().ui.log_actions) {
      const ignored_actions = [
        'START_LOADING',
        'STOP_LOADING',
      ];

      // Show non-ignored actions
      if (!ignored_actions.includes(action.type)) {
        console.log(action);
      }
    }

    switch (action.type) {
      case 'PUSHER_CONNECTED':
        storage.set(
          'pusher',
          {
            connection_id: action.connection_id,
          },
        );
        break;

      case 'PUSHER_SET_PORT':
        storage.set(
          'pusher',
          {
            port: action.port,
          },
        );
        break;

      case 'PUSHER_SET_USERNAME':
        storage.set(
          'pusher',
          {
            username: action.username,
          },
        );
        break;

      case 'MOPIDY_URISCHEMES_FILTERED':
        storage.set(
          'mopidy',
          {
            uri_schemes: action.data,
          },
        );
        break;

      case 'MOPIDY_UPDATE_SERVERS':
        storage.set(
          'mopidy',
          {
            servers: action.servers,
          },
        );
        break;

      case 'SPOTIFY_AUTHORIZATION_GRANTED':
        if (action.authorization !== undefined) {
          var { authorization } = action;
        } else if (action.data) {
          var authorization = action.data;
        }
        storage.set(
          'spotify',
          {
            authorization,
            access_token: authorization.access_token,
            refresh_token: authorization.refresh_token,
            token_expiry: authorization.token_expiry,
          },
        );
        break;

      case 'SPOTIFY_IMPORT_AUTHORIZATION':
        storage.set(
          'spotify',
          {
            authorization: action.authorization,
            access_token: action.authorization.access_token,
            refresh_token: action.authorization.refresh_token,
            token_expiry: action.authorization.token_expiry,
            me: undefined,
          },
        );
        break;

      case 'SPOTIFY_AUTHORIZATION_REVOKED':
        storage.set(
          'spotify',
          {
            authorization: undefined,
            access_token: undefined,
            refresh_token: undefined,
            token_expiry: undefined,
            me: undefined,
          },
        );
        break;

      case 'SPOTIFY_TOKEN_REFRESHED':
        storage.set(
          'spotify',
          {
            access_token: action.data.access_token,
            token_expiry: action.data.token_expiry,
            provider: action.provider,
          },
        );
        break;

      case 'SPOTIFY_ME_LOADED':
        storage.set(
          'spotify',
          {
            me: action.me,
          },
        );
        break;

      case 'LASTFM_ME_LOADED':
        storage.set(
          'lastfm',
          {
            me: action.me,
          },
        );
        break;

      case 'LASTFM_AUTHORIZATION_GRANTED':
        storage.set(
          'lastfm',
          {
            authorization: action.data.session,
          },
        );
        break;

      case 'LASTFM_AUTHORIZATION_REVOKED':
        storage.set(
          'lastfm',
          {
            authorization: undefined,
            me: undefined,
          },
        );
        break;

      case 'LASTFM_IMPORT_AUTHORIZATION':
        storage.set(
          'lastfm',
          {
            authorization: action.authorization,
            me: undefined,
          },
        );
        break;

      case 'GENIUS_ME_LOADED':
        storage.set(
          'genius',
          {
            me: action.me,
          },
        );
        break;

      case 'GENIUS_AUTHORIZATION_GRANTED':
        storage.set(
          'genius',
          {
            authorization: action.data,
            authorization_code: action.data.authorization_code,
            access_token: action.data.access_token,
          },
        );
        break;

      case 'GENIUS_AUTHORIZATION_REVOKED':
        storage.set(
          'genius',
          {
            me: undefined,
            authorization: undefined,
            authorization_code: undefined,
            access_token: undefined,
          },
        );
        break;

      case 'GENIUS_IMPORT_AUTHORIZATION':
        storage.set(
          'genius',
          {
            authorization: action.authorization,
            authorization_code: action.authorization.authorization_code,
            access_token: action.authorization.access_token,
            me: undefined,
          },
        );
        break;

      case 'CORE_SET':
        storage.set(
          'core',
          action.data,
        );
        break;

      case 'UPDATE_PINNED':
        storage.set(
          'core',
          { pinned: action.pinned },
        );
        break;

      case 'UI_SET':
        storage.set(
          'ui',
          action.data,
        );
        break;

      case 'MOPIDY_SET':
        storage.set(
          'mopidy',
          action.data,
        );
        break;

      case 'SPOTIFY_SET':
        storage.set(
          'spotify',
          action.data,
        );
        break;

      case 'SNAPCAST_SET':
        storage.set(
          'snapcast',
          action.data,
        );
        break;

      case 'SNAPCAST_COMMANDS_UPDATED':
        storage.set(
          'snapcast',
          {
            commands: action.commands,
          },
        );
        break;
/*
      case 'SUPPRESS_BROADCAST':
        var ui = storage.get('ui');
        if (ui.suppressed_broadcasts !== undefined) {
          var { suppressed_broadcasts } = ui;
        } else {
          var suppressed_broadcasts = [];
        }

        suppressed_broadcasts.push(action.key);

        storage.set(
          'ui',
          {
            suppressed_broadcasts,
          },
        );
        break;
        */

        /**
         * Experimental saving of stores to localStorage
         * This uses way too much storage space (ie 10MB+) so won't work. We need
         * to use the IndexedDB engine instead for storing this quantity of data

        case 'UPDATE_TRACKS_INDEX':
            storage.set('core', {tracks: action.tracks});
            next(action);
            break;
        case 'UPDATE_ALBUMS_INDEX':
            storage.set('core', {albums: action.albums});
            next(action);
            break;
        case 'UPDATE_ARTISTS_INDEX':
            storage.set('core', {artists: action.artists});
            next(action);
            break;
        case 'UPDATE_PLAYLISTS_INDEX':
            storage.set('core', {playlists: action.playlists});
            next(action);
            break;
        case 'UPDATE_USERS_INDEX':
            storage.set('core', {users: action.users});
            next(action);
            break;
          */

      default:
        break;
    }
  };
}());

export default localstorageMiddleware;

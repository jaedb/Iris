import { createStore, applyMiddleware, combineReducers } from 'redux';
import { persistStore, persistReducer } from 'redux-persist';
import localForage from 'localforage';
import thunk from 'redux-thunk';

import { generateGuid } from '../util/helpers';
import core from '../services/core/reducer';
import ui from '../services/ui/reducer';
import pusher from '../services/pusher/reducer';
import mopidy from '../services/mopidy/reducer';
import lastfm from '../services/lastfm/reducer';
import spotify from '../services/spotify/reducer';
import snapcast from '../services/snapcast/reducer';
import genius from '../services/genius/reducer';

import migration from './migration';

import coreMiddleware from '../services/core/middleware';
import uiMiddleware from '../services/ui/middleware';
import pusherMiddleware from '../services/pusher/middleware';
import mopidyMiddleware from '../services/mopidy/middleware';
import lastfmMiddleware from '../services/lastfm/middleware';
import geniusMiddleware from '../services/genius/middleware';
import spotifyMiddleware from '../services/spotify/middleware';
import snapcastMiddleware from '../services/snapcast/middleware';

let initialState = {
  core: {
    outputs: [],
    queue: [],
    queue_metadata: {},
    current_track: null,
    pinned: [],
    albums: {},
    artists: {},
    playlists: {},
    users: {},
    tracks: {},
    items: {},
    libraries: {},
  },
  ui: {
    language: 'en',
    theme: 'auto',
    smooth_scrolling_enabled: true,
    hotkeys_enabled: true,
    playback_controls_touch_enabled: true,
    allow_reporting: true,
    wide_scrollbar_enabled: false,
    window_focus: true,
    slim_mode: false,
    selected_tracks: [],
    notifications: {},
    processes: {},
    suppressed_broadcasts: [],
    grid_glow_enabled: true,
  },
  mopidy: {
    connected: false,
    host: window.location.hostname,
    port: (window.location.port ? window.location.port : (window.location.protocol === 'https:' ? '443' : '80')),
    ssl: window.location.protocol === 'https:',
    current_server: 'default',
    servers: {
      default: {
        id: 'default',
        name: 'Default',
        host: window.location.hostname,
        port: (window.location.port ? window.location.port : (window.location.protocol === 'https:' ? '443' : '80')),
        ssl: window.location.protocol === 'https:',
      },
    },
    mute: false,
    volume: 0,
    progress: 0,
    play_state: null,
    uri_schemes: [],
    library_albums_uri: 'local:directory?type=album',
    library_artists_uri: 'local:directory?type=artist&role=albumartist',
    library_tracks_uri: 'local:directory?type=track',
  },
  pusher: {
    connected: false,
    username: generateGuid(),
    client_id: generateGuid(),
    connections: {},
    version: {
      current: null,
    },
    config: {},
  },
  lastfm: {
    authorization_url: 'https://jamesbarnsley.co.nz/iris/auth_lastfm.php',
  },
  genius: {
    authorization_url: 'https://jamesbarnsley.co.nz/iris/auth_genius.php',
  },
  spotify: {
    autocomplete_results: {},
    authorization_url: 'https://jamesbarnsley.co.nz/iris/auth_spotify.php',
  },
  snapcast: {
    enabled: false,
    connected: false,
    host: window.location.hostname,
    port: '1780',
    ssl: (window.location.protocol === 'https:'),
    streams: {},
    groups: {},
    clients: {},
    server: null,
    commands: {},
  },
};

// Run any migrations
initialState = migration(initialState);

const corePersistConfig = {
  key: 'core',
  storage: localForage,
  debug: window.test_mode,
  blacklist: [
    'items',
    'albums',
    'artists',
    'playlists',
    'search_results',
    'users',
    'tracks',
    'libraries', // We manually hydrate this, so we can handle the rehydration of library items
  ],
};

const geniusPersistConfig = {
  key: 'genius',
  storage: localForage,
  debug: window.test_mode,
};

const lastfmPersistConfig = {
  key: 'lastfm',
  storage: localForage,
  debug: window.test_mode,
};

const snapcastPersistConfig = {
  key: 'snapcast',
  storage: localForage,
  debug: window.test_mode,
};

const pusherPersistConfig = {
  key: 'pusher',
  storage: localForage,
  blacklist: [
    'connected',
    'connections',
  ],
  debug: window.test_mode,
};

const mopidyPersistConfig = {
  key: 'mopidy',
  storage: localForage,
  debug: window.test_mode,
  blacklist: [
    'connected',
  ],
};

const spotifyPersistConfig = {
  key: 'spotify',
  storage: localForage,
  debug: window.test_mode,
  whitelist: [
    'access_token',
    'authorization',
    'country',
    'enabled',
    'locale',
    'me',
    'refresh_token',
    'token_expiry',
  ],
};

const uiPersistConfig = {
  key: 'ui',
  storage: localForage,
  blacklist: [
    'debug_response',
    'load_queue',
    'notifications',
    'processes',
    'context_menu',
    'current_track_transition',
    'dragger',
    'selected_tracks',
    'sidebar_open',
    'window_focus',
  ],
  debug: window.test_mode,
};

const appReducer = combineReducers({
  core: persistReducer(corePersistConfig, core),
  ui: persistReducer(uiPersistConfig, ui),
  mopidy: persistReducer(mopidyPersistConfig, mopidy),
  spotify: persistReducer(spotifyPersistConfig, spotify),
  pusher: persistReducer(pusherPersistConfig, pusher),
  genius: persistReducer(geniusPersistConfig, genius),
  lastfm: persistReducer(lastfmPersistConfig, lastfm),
  snapcast: persistReducer(snapcastPersistConfig, snapcast),
});
const rootReducer = (state, action) => {
  let nextState = state;
  if (action.type === 'RESET_STATE') {
    console.log(action);
    const { stateKeysToReset } = action;
    const resetStates = {};
    stateKeysToReset.forEach((key) => {
      resetStates[key] = initialState[key];
    });
    nextState = {
      ...state,
      ...resetStates,
    };
  }
  return appReducer(nextState, action);
};

const store = createStore(
  rootReducer,
  initialState,
  applyMiddleware(
    thunk,
    coreMiddleware,
    uiMiddleware,
    mopidyMiddleware,
    pusherMiddleware,
    spotifyMiddleware,
    lastfmMiddleware,
    geniusMiddleware,
    snapcastMiddleware,
  ),
);
const persistor = persistStore(store);

export default { store, persistor };
export { store, persistor };

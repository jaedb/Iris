
import { createStore, applyMiddleware, combineReducers } from 'redux';
import thunk from 'redux-thunk';

import { generateGuid } from '../util/helpers';
import storage from '../util/storage';
import core from '../services/core/reducer';
import ui from '../services/ui/reducer';
import pusher from '../services/pusher/reducer';
import mopidy from '../services/mopidy/reducer';
import lastfm from '../services/lastfm/reducer';
import spotify from '../services/spotify/reducer';
import snapcast from '../services/snapcast/reducer';
import google from '../services/google/reducer';
import genius from '../services/genius/reducer';

import migration from './migration';

import coreMiddleware from '../services/core/middleware';
import uiMiddleware from '../services/ui/middleware';
import pusherMiddleware from '../services/pusher/middleware';
import mopidyMiddleware from '../services/mopidy/middleware';
import lastfmMiddleware from '../services/lastfm/middleware';
import geniusMiddleware from '../services/genius/middleware';
import spotifyMiddleware from '../services/spotify/middleware';
import googleMiddleware from '../services/google/middleware';
import snapcastMiddleware from '../services/snapcast/middleware';
import localstorageMiddleware from '../services/localstorage/middleware';

let state = {
  core: {
    outputs: [],
    queue: [],
    queue_metadata: {},
    current_track: null,
    albums: {},
    artists: {},
    playlists: {},
    users: {},
    tracks: {},
    http_streaming_enabled: false,
    http_streaming_cachebuster: null,
    http_streaming_url: `http://${window.location.hostname}:8000/mopidy`,
  },
  ui: {
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
    me: null,
    authorization_url: 'https://jamesbarnsley.co.nz/iris/auth_lastfm.php',
  },
  genius: {
    me: null,
    authorization_url: 'https://jamesbarnsley.co.nz/iris/auth_genius.php',
  },
  spotify: {
    me: null,
    autocomplete_results: {},
    authorization_url: 'https://jamesbarnsley.co.nz/iris/auth_spotify.php',
  },
  google: {
    enabled: false,
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

// load all our stored values from LocalStorage
state.core = { ...state.core, ...storage.get('core') };
state.ui = { ...state.ui, ...storage.get('ui') };
state.mopidy = { ...state.mopidy, ...storage.get('mopidy') };
state.pusher = { ...state.pusher, ...storage.get('pusher') };
state.spotify = { ...state.spotify, ...storage.get('spotify') };
state.lastfm = { ...state.lastfm, ...storage.get('lastfm') };
state.genius = { ...state.genius, ...storage.get('genius') };
state.google = { ...state.google, ...storage.get('google') };
state.snapcast = { ...state.snapcast, ...storage.get('snapcast') };

// Run any migrations
state = migration(state);

const reducers = combineReducers({
  core,
  ui,
  pusher,
  mopidy,
  lastfm,
  genius,
  spotify,
  google,
  snapcast,
});

export default createStore(
  reducers,
  state,
  applyMiddleware(
    thunk,
    localstorageMiddleware,
    coreMiddleware,
    uiMiddleware,
    mopidyMiddleware,
    pusherMiddleware,
    spotifyMiddleware,
    lastfmMiddleware,
    geniusMiddleware,
    googleMiddleware,
    snapcastMiddleware,
  ),
);


import { createStore, applyMiddleware, combineReducers } from 'redux';

import * as helpers from './helpers';

import core from './services/core/reducer';
import ui from './services/ui/reducer';
import pusher from './services/pusher/reducer';
import mopidy from './services/mopidy/reducer';
import lastfm from './services/lastfm/reducer';
import spotify from './services/spotify/reducer';
import snapcast from './services/snapcast/reducer';
import google from './services/google/reducer';
import genius from './services/genius/reducer';

import storeMigration from './storeMigration';

import thunk from 'redux-thunk';
import coreMiddleware from './services/core/middleware';
import uiMiddleware from './services/ui/middleware';
import pusherMiddleware from './services/pusher/middleware';
import mopidyMiddleware from './services/mopidy/middleware';
import lastfmMiddleware from './services/lastfm/middleware';
import geniusMiddleware from './services/genius/middleware';
import spotifyMiddleware from './services/spotify/middleware';
import googleMiddleware from './services/google/middleware';
import snapcastMiddleware from './services/snapcast/middleware';
import localstorageMiddleware from './services/localstorage/middleware';

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
		http_streaming_url: "http://"+window.location.hostname+":8000/mopidy"
	},
	ui: {
		theme: 'dark',
		smooth_scrolling_enabled: true,
		hotkeys_enabled: true,
		allow_reporting: true,
		window_focus: true,
		slim_mode: false,
		selected_tracks: [],
		notifications: {},
		processes: {}
	},
	mopidy: {
		connected: false,
		host: window.location.hostname,
		port: (window.location.port ? window.location.port : (window.location.protocol === 'https:' ? '443' : '80')),
		ssl: (window.location.protocol === 'https:' ? true : false),
		mute: false,
		volume: 0,
		progress: 0,
		play_state: null,
		uri_schemes: [],
		library_albums_uri: 'local:directory?type=album',
		library_artists_uri: 'local:directory?type=artist'
	},
	pusher: {
		connected: false,
		username: null,
		connections: {},
		version: {
			current: null
		},
		config: {}
	},
	lastfm: {
		me: null,
		authorization_url: 'https://jamesbarnsley.co.nz/iris/auth_lastfm.php'
	},
	genius: {
		me: null,
		authorization_url: 'https://jamesbarnsley.co.nz/iris/auth_genius.php'
	},
	spotify: {
		me: null,
		autocomplete_results: {},
		authorization_url: 'https://jamesbarnsley.co.nz/iris/auth_spotify.php'
	},
	google: {
		enabled: false
	},
	snapcast: {
		streams: {},
		groups: {},
		clients: {},
		server: null,
		commands: {}
	}
};

// load all our stored values from LocalStorage
state.core = Object.assign({}, state.core, helpers.getStorage('core'));
state.ui = Object.assign({}, state.ui, helpers.getStorage('ui'));
state.mopidy = Object.assign({}, state.mopidy, helpers.getStorage('mopidy'));
state.pusher = Object.assign({}, state.pusher, helpers.getStorage('pusher'));
state.spotify = Object.assign({}, state.spotify, helpers.getStorage('spotify'));
state.lastfm = Object.assign({}, state.lastfm, helpers.getStorage('lastfm'));
state.genius = Object.assign({}, state.genius, helpers.getStorage('genius'));
state.google = Object.assign({}, state.google, helpers.getStorage('google'));
state.snapcast = Object.assign({}, state.snapcast, helpers.getStorage('snapcast'));

// Run any migrations
state = storeMigration(state);

const reducers = combineReducers({
    core,
    ui,
    pusher,
    mopidy,
    lastfm,
    genius,
    spotify,
    google,
    snapcast
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
		snapcastMiddleware
	)
);

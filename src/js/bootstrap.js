
import { createStore, applyMiddleware, combineReducers } from 'redux'

import * as helpers from './helpers'

import core from './services/core/reducer'
import ui from './services/ui/reducer'
import pusher from './services/pusher/reducer'
import mopidy from './services/mopidy/reducer'
import lastfm from './services/lastfm/reducer'
import spotify from './services/spotify/reducer'
import genius from './services/genius/reducer'

import thunk from 'redux-thunk'
import coreMiddleware from './services/core/middleware'
import uiMiddleware from './services/ui/middleware'
import pusherMiddleware from './services/pusher/middleware'
import mopidyMiddleware from './services/mopidy/middleware'
import lastfmMiddleware from './services/lastfm/middleware'
import spotifyMiddleware from './services/spotify/middleware'
import localstorageMiddleware from './services/localstorage/middleware'

let reducers = combineReducers({
    core,
    ui,
    pusher,
    mopidy,
    lastfm,
    genius,
    spotify
});

// set application defaults
// TODO: Look at using propTypes in the component for these falsy initial states
var initialState = {
	core: {
		queue: [],
		queue_metadata: {},
		current_track_uri: null,
		albums: {},
		artists: {},
		playlists: {},
		users: {},
		tracks: {},
		http_streaming_enabled: false,
		http_streaming_url: "http://"+window.location.hostname+":8000/mopidy"
	},
	ui: {
		show_initial_setup: true,
		slim_mode: false,
		selected_tracks: [],
		notifications: [],
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
		play_state: false
	},
	pusher: {
		connected: false,
		username: false,
		connections: {},
		version: {
			current: '0.0.0'
		}
	},
	lastfm: {
		connected: false,
		me: false,
		authorization_url: 'https://jamesbarnsley.co.nz/auth_lastfm.php'
	},
	genius: {
		connected: false
	},
	spotify: {
		connected: false,
		me: false,
		autocomplete_results: {},
		authorization_url: 'https://jamesbarnsley.co.nz/auth_spotify.php'
	}
};

// load all our stored values from LocalStorage
initialState.core = Object.assign({}, initialState.core, helpers.getStorage('core'));
initialState.ui = Object.assign({}, initialState.ui, helpers.getStorage('ui'));
initialState.mopidy = Object.assign({}, initialState.mopidy, helpers.getStorage('mopidy'));
initialState.pusher = Object.assign({}, initialState.pusher, helpers.getStorage('pusher'));
initialState.spotify = Object.assign({}, initialState.spotify, helpers.getStorage('spotify'));
initialState.lastfm = Object.assign({}, initialState.lastfm, helpers.getStorage('lastfm'));

console.log('Bootstrapping', initialState)

let store = createStore(
	reducers, 
	initialState, 
	applyMiddleware(thunk, localstorageMiddleware, coreMiddleware, uiMiddleware, mopidyMiddleware, pusherMiddleware, spotifyMiddleware, lastfmMiddleware )
);

export default store;

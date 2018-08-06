
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
import persistenceMiddleware from './services/persistence/middleware'

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
		shortkeys_enabled: true,
		allow_reporting: true,
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
		play_state: false,
		uri_schemes: [],
		library_albums_uri: 'local:directory?type=album',
		library_artists_uri: 'local:directory?type=artist'
	},
	pusher: {
		connected: false,
		username: false,
		connections: {},
		version: {
			current: '0.0.0'
		},
		config: {}
	},
	lastfm: {
		me: false,
		authorization_url: 'https://jamesbarnsley.co.nz/iris/auth_lastfm.php'
	},
	genius: {
		provider_url: 'https://james.barnsley.nz/iris/provider_genius.php'
	},
	spotify: {
		me: false,
		autocomplete_results: {},
		authorization_url: 'https://jamesbarnsley.co.nz/iris/auth_spotify.php'
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
	applyMiddleware(thunk, persistenceMiddleware, coreMiddleware, uiMiddleware, mopidyMiddleware, pusherMiddleware, spotifyMiddleware, lastfmMiddleware )
);

export default store;

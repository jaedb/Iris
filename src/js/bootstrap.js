
import { createStore, applyMiddleware, combineReducers } from 'redux'

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
		current_tracklist: [],
		current_tltrack: false,
		albums: {},
		artists: {},
		playlists: {},
		users: {},
		tracks: {}
	},
	ui: {
		slim_mode: false,
		selected_tracks: [],
		notifications: [],
		processes: {}
	},
	mopidy: {
		connected: false,
		host: window.location.hostname,
		port: (window.location.port ? window.location.port : (window.location.protocol === 'https:' ? '443' : '80')),
		mute: false,
		volume: 0,
		progress: 0,
		play_state: false
	},
	pusher: {
		connected: false,
		username: 'Anonymous',
		connections: {},
		version: {
			current: '0.0.0'
		}
	},
	lastfm: {
		connected: false
	},
	genius: {
		connected: false
	},
	spotify: {
		connected: false,
		me: false,
		autocomplete_results: {},
		authorization_url: 'https://jamesbarnsley.co.nz/auth_v2.php'
	}
};

// if we've got a stored version of spotify state, load and merge
if( localStorage.getItem('core') ){
	var storedCore = JSON.parse( localStorage.getItem('core') );
	initialState.core = Object.assign(initialState.core, storedCore );
}

// if we've got a stored version of spotify state, load and merge
if( localStorage.getItem('ui') ){
	var storedUi = JSON.parse( localStorage.getItem('ui') );
	initialState.ui = Object.assign(initialState.ui, storedUi );
}

// if we've got a stored version of mopidy state, load and merge
if( localStorage.getItem('mopidy') ){
	var storedMopidy = JSON.parse( localStorage.getItem('mopidy') );
	initialState.mopidy = Object.assign(initialState.mopidy, storedMopidy );
}

// if we've got a stored version of pusher state, load and merge
if( localStorage.getItem('pusher') ){
	var storedPusher = JSON.parse( localStorage.getItem('pusher') );
	initialState.pusher = Object.assign(initialState.pusher, storedPusher );
}

// if we've got a stored version of spotify state, load and merge
if( localStorage.getItem('spotify') ){
	var storedSpotify = JSON.parse( localStorage.getItem('spotify') );
	initialState.spotify = Object.assign(initialState.spotify, storedSpotify );
}

console.log('Bootstrapping', initialState)

let store = createStore(
	reducers, 
	initialState, 
	applyMiddleware( thunk, localstorageMiddleware, coreMiddleware, uiMiddleware, mopidyMiddleware, pusherMiddleware, spotifyMiddleware )
);

export default store;

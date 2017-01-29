
import { createStore, applyMiddleware, combineReducers } from 'redux'

import ui from './services/ui/reducer'
import pusher from './services/pusher/reducer'
import mopidy from './services/mopidy/reducer'
import lastfm from './services/lastfm/reducer'
import spotify from './services/spotify/reducer'

import thunk from 'redux-thunk'
import uiMiddleware from './services/ui/middleware'
import pusherMiddleware from './services/pusher/middleware'
import mopidyMiddleware from './services/mopidy/middleware'
import spotifyMiddleware from './services/spotify/middleware'
import localstorageMiddleware from './services/localstorage/middleware'

let reducers = combineReducers({
    ui,
    pusher,
    mopidy,
    lastfm,
    spotify
});

// set application defaults
// TODO: Look at using propTypes in the component for these falsy initial states
var initialState = {
	mopidy: {
		connected: false,
		host: window.location.hostname,
		port: 6680,
		volume: 0,
		progress: 0,
		play_state: false
	},
	pusher: {
		connected: false,
		port: 6681,
		username: 'Anonymous',
		connections: [],
		version: {
			current: '0.0.0'
		}
	},
	lastfm: {
		album: {},
		artist: {},
		track: {}
	},
	spotify: {
		connected: false,
		country: 'NZ',
		locale: 'en_NZ',
		me: false
	},
	ui: {
		current_tracklist: [],
		current_tltrack: false,
		notifications: []
	}
};

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

// if we've got a stored version of spotify state, load and merge
if( localStorage.getItem('ui') ){
	var storedUi = JSON.parse( localStorage.getItem('ui') );
	initialState.ui = Object.assign(initialState.ui, storedUi );
}

console.log('Bootstrapping', initialState)

let store = createStore(
	reducers, 
	initialState, 
	applyMiddleware( thunk, localstorageMiddleware, uiMiddleware, mopidyMiddleware, pusherMiddleware, spotifyMiddleware )
);

export default store;

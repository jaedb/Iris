
console.info('Bootstrapping...');

import { createStore, applyMiddleware, combineReducers } from 'redux'

import ui from './services/ui/reducer'
import mopidy from './services/mopidy/reducer'
import spotify from './services/spotify/reducer'

import thunk from 'redux-thunk'
import mopidyMiddleware from './services/mopidy/middleware'
import spotifyMiddleware from './services/spotify/middleware'
import localstorageMiddleware from './services/localstorage/middleware'

let reducers = combineReducers({
    ui,
    mopidy,
    spotify
});

// set application defaults
var initialState = {
	mopidy: {
		host: window.location.hostname,
		port: 6680,
		volume: 0,
		progress: 0
	},
	spotify: {
		country: 'NZ',
		locale: 'en_NZ',
		me: false
	},
	ui: {
		context_menu: {
			show: false
		}
	}
};

// if we've got a stored version of mopidy state, load and merge
if( localStorage.getItem('mopidy') ){
	var storedMopidy = JSON.parse( localStorage.getItem('mopidy') );
	initialState.mopidy = Object.assign(initialState.mopidy, storedMopidy );
}

// if we've got a stored version of spotify state, load and merge
if( localStorage.getItem('spotify') ){
	var storedSpotify = JSON.parse( localStorage.getItem('spotify') );
	initialState.spotify = Object.assign(initialState.spotify, storedSpotify );
}

let store = createStore(
	reducers, 
	initialState, 
	applyMiddleware( thunk, localstorageMiddleware, mopidyMiddleware, spotifyMiddleware )
);

export default store;

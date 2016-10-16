
console.info('Bootstrapping...');

import { createStore, applyMiddleware, combineReducers } from 'redux'

import mopidy from './services/mopidy/reducer'
import spotify from './services/spotify/reducer'

import thunk from 'redux-thunk'
import mopidyMiddleware from './services/mopidy/middleware'
import spotifyMiddleware from './services/spotify/middleware'
import localstorageMiddleware from './services/localstorageMiddleware'

let reducers = combineReducers({
    mopidy,
    spotify
});

// load our state from localStorage
var initialState = {};
if( localStorage.getItem('state') ){
	initialState = JSON.parse( localStorage.getItem('state') );
}

let store = createStore(
	reducers, 
	initialState, 
	applyMiddleware( thunk, localstorageMiddleware, mopidyMiddleware, spotifyMiddleware )
);

export default store;

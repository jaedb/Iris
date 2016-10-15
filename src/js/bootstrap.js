
console.info('Bootstrapping...');

import { createStore, applyMiddleware, combineReducers } from 'redux'

import mopidy from './services/mopidy/reducer'
import spotify from './services/spotify/reducer'

import thunk from 'redux-thunk'
import mopidyMiddleware from './services/mopidy/middleware'
import spotifyMiddleware from './services/spotify/middleware'

let reducers = combineReducers({
    mopidy,
    spotify
});

let store = createStore(
	reducers, 
	{}, 
	applyMiddleware( thunk, mopidyMiddleware, spotifyMiddleware )
);

export default store;

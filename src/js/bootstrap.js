
console.info('Bootstrapping...');

import { createStore, applyMiddleware } from 'redux'
import thunk from 'redux-thunk'
import reducer from './reducers/index'
import mopidyMiddleware from './services/mopidy/middleware'

// create our global store
let store = createStore(
	reducer, 
	{}, 
	applyMiddleware( thunk, mopidyMiddleware )
);

export default store;

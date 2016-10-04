
console.info('Bootstrapping...');

import { createStore } from 'redux'
import reducer from './reducers/index'

// create our global store
let store = createStore( reducer, {} );

export default store;

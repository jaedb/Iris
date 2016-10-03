
console.info('Bootstrapping...');

import { createStore } from 'redux'
import reducer from './reducers/index'
import Services from './services/Services'

// start all our services
Promise.all(Services.setup())
	.then(() => {
		console.info('Services started...');
	});

// create our global store
let store = createStore( reducer, {} );

export default store;

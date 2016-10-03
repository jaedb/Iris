
console.info('Bootstrapping...');

import { createStore } from 'redux'
import reducer from './reducers/index'
import Services from './services/Services'

// create our global store
let store = createStore( reducer, {} );

// start all our services
Promise.all(Services.setup( store ))
	.then(() => {
		console.info('Services started...');
	});

export default store;

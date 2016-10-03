/**
 * Base-level application wrapper
 **/
 
console.info('Application initiated');


import React, { PropTypes } from 'react';
import ReactDOM from 'react-dom';
import { Provider } from 'react-redux'
import { createStore } from 'redux'
import { Router, Route, Link, hashHistory } from 'react-router'

import store from './bootstrap.js'
import App from './components/App'
import Album from './components/Album'
import Queue from './components/Queue'

import Services from './services/Services'

Promise.all(Services.setup())
	.then(() => {
		console.info('Services started...');
	});


/**
 * Render our application into the real DOM
 *
 * Sets up all of our routes, and the relevant components for each.
 * Provider facilitates global access to the store by using connect()

				<Route path="playlists" component={LibraryPlaylists} />			
				<Route path="albums" component={LibraryAlbums} />
				<Route path="artists" component={LibraryArtists} />
				<Route path="tracks" component={LibraryTracks} />
 **/

ReactDOM.render(
	<Provider store={store}>
		<Router history={hashHistory}>
			<Route path="/" component={App}>
	
				<Route path="album/:id" component={Album} />
				<Route path="queue" component={Queue} />

			</Route>
		</Router>
	</Provider>,
	document.getElementById('app')
);
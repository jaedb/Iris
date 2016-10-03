/**
 * Base-level application wrapper
 **/
 
console.info('Application initiated');


import React, { PropTypes } from 'react';
import ReactDOM from 'react-dom';
import { Provider } from 'react-redux'
import { createStore } from 'redux'
import { Router, Route, Link, hashHistory } from 'react-router'
import reducer from './reducer'

import App from './App'
import LibraryAlbums from './library/LibraryAlbums'
import Album from './common/Album'
import NowPlaying from './routes/NowPlaying'

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
	<Provider store={ createStore( reducer, { todos: [] } ) }>
		<Router history={hashHistory}>
			<Route path="/" component={App}>
	
				<Route path="album/:id" component={Album} />
				<Route path="library/albums" component={LibraryAlbums} />

				<Route path="now-playing" component={NowPlaying} />

			</Route>
		</Router>
	</Provider>,
	document.getElementById('app')
);
/**
 * Base-level application wrapper
 **/

import React, { PropTypes } from 'react';
import ReactDOM from 'react-dom';
import { Provider } from 'react-redux'
import { createStore } from 'redux'
import { Router, Route, Link, hashHistory } from 'react-router'

import store from './bootstrap.js'

require('../scss/app.scss');

import App from './views/App'
import Album from './views/Album'
import Artist from './views/Artist'
import Queue from './views/Queue'
import Settings from './views/Settings'
import LibraryArtists from './views/LibraryArtists'

ReactDOM.render(
	<Provider store={store}>
		<Router history={hashHistory}>
			<Route path="/" component={App}>
	
				<Route path="library/artists" component={LibraryArtists} />
				<Route path="album/:uri" component={Album} />
				<Route path="artist/:uri" component={Artist} />
				<Route path="queue" component={Queue} />
				<Route path="settings" component={Settings} />

			</Route>
		</Router>
	</Provider>,
	document.getElementById('app')
);
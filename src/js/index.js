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
import Playlist from './views/Playlist'
import Queue from './views/Queue'
import Settings from './views/Settings'

import LibraryArtists from './views/library/LibraryArtists'
import LibraryAlbums from './views/library/LibraryAlbums'
import LibraryTracks from './views/library/LibraryTracks'
import LibraryPlaylists from './views/library/LibraryPlaylists'

ReactDOM.render(
	<Provider store={store}>
		<Router history={hashHistory}>
			<Route path="/" component={App}>
	
				<Route path="library/artists" component={LibraryArtists} />
				<Route path="library/albums" component={LibraryAlbums} />
				<Route path="library/tracks" component={LibraryTracks} />
				<Route path="library/playlists" component={LibraryPlaylists} />
				
				<Route path="album/:uri" component={Album} />
				<Route path="artist/:uri" component={Artist} />
				<Route path="playlist/:uri" component={Playlist} />
				<Route path="queue" component={Queue} />
				<Route path="settings" component={Settings} />

			</Route>
		</Router>
	</Provider>,
	document.getElementById('app')
);
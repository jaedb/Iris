/**
 * Base-level application wrapper
 **/

import React, { PropTypes } from 'react';
import ReactDOM from 'react-dom';
import { Provider } from 'react-redux'
import { createStore } from 'redux'
import { Router, Route, Link, IndexRoute, hashHistory } from 'react-router'
import ReactGA from 'react-ga'

import store from './bootstrap.js'
require('../scss/app.scss');

import App from './App'
import Album from './views/Album'
import Artist from './views/Artist'
import Playlist from './views/Playlist'
import User from './views/User'
import Queue from './views/Queue'
import QueueHistory from './views/QueueHistory'
import Settings from './views/Settings'
import Debug from './views/Debug'
import Search from './views/Search'

import DiscoverRecommendations from './views/discover/DiscoverRecommendations'
import DiscoverFeatured from './views/discover/DiscoverFeatured'
import DiscoverCategories from './views/discover/DiscoverCategories'
import DiscoverCategory from './views/discover/DiscoverCategory'
import DiscoverNewReleases from './views/discover/DiscoverNewReleases'

import LibraryArtists from './views/library/LibraryArtists'
import LibraryAlbums from './views/library/LibraryAlbums'
import LibraryTracks from './views/library/LibraryTracks'
import LibraryPlaylists from './views/library/LibraryPlaylists'
import LibraryBrowse from './views/library/LibraryBrowse'

/*
// Hijack console error for Raven to capture
var originalConsoleError = console.error;  
console.error = function(message, error){  
    Raven.captureException(message, error);
    originalConsoleError.apply(this, arguments)
*/

// setup our analytics tracking
ReactGA.initialize('UA-64701652-3');
function handleUpdate() {
	ReactGA.set({ page: window.location.hash })
	ReactGA.pageview(window.location.hash)
	$(window).scrollTop(0)
	store.dispatch({type: 'HIDE_CONTEXT_MENU'})
}

global.baseURL = '/'

ReactDOM.render(
	<Provider store={store}>
		<Router history={hashHistory} onUpdate={handleUpdate}>
			<Route path={global.baseURL} component={App}>

     			<IndexRoute component={Queue} />
				<Route path="queue" component={Queue} />
				<Route path="queue/history" component={QueueHistory} />
				<Route path="settings" component={Settings} />
				<Route path="settings/debug" component={Debug} />
				
				<Route path="search(/iris::search::query)" component={Search} />
				<Route path="album/:uri" component={Album} />
				<Route path="artist/:uri(/:sub_view)" component={Artist} />
				<Route path="playlist/:uri" component={Playlist} />
				<Route path="user/:uri" component={User} />
	
				<Route path="discover/recommendations(/:seeds)" component={DiscoverRecommendations} />
				<Route path="discover/featured" component={DiscoverFeatured} />
				<Route path="discover/categories" component={DiscoverCategories} />
				<Route path="discover/categories/:id" component={DiscoverCategory} />
				<Route path="discover/new-releases" component={DiscoverNewReleases} />

				<Route path="library/artists" component={LibraryArtists} />
				<Route path="library/albums" component={LibraryAlbums} />
				<Route path="library/tracks" component={LibraryTracks} />
				<Route path="library/playlists" component={LibraryPlaylists} />
				<Route path="library/browse(/:uri)" component={LibraryBrowse} />

			</Route>
		</Router>
	</Provider>,
	document.getElementById('app')
);
/**
 * Base-level application wrapper
 **/

import React, { PropTypes } from 'react';
import ReactDOM from 'react-dom';
import { Provider } from 'react-redux'
import { createStore } from 'redux'
import { Router, Route, Link, IndexRoute, hashHistory } from 'react-router'

import store from './bootstrap.js'

require('../scss/app.scss');

import App from './views/App'
import Album from './views/Album'
import Artist from './views/Artist'
import Playlist from './views/Playlist'
import User from './views/User'
import Queue from './views/Queue'
import Settings from './views/Settings'
import Debug from './views/Debug'
import Search from './views/Search'

import Discover from './views/discover/Discover'
import DiscoverFeatured from './views/discover/DiscoverFeatured'
import DiscoverCategories from './views/discover/DiscoverCategories'
import DiscoverCategory from './views/discover/DiscoverCategory'
import DiscoverNewReleases from './views/discover/DiscoverNewReleases'

import LibraryArtists from './views/library/LibraryArtists'
import LibraryAlbums from './views/library/LibraryAlbums'
import LibraryTracks from './views/library/LibraryTracks'
import LibraryPlaylists from './views/library/LibraryPlaylists'
import LibraryLocal from './views/library/LibraryLocal'
import LibraryLocalArtists from './views/library/LibraryLocalArtists'
import LibraryLocalAlbums from './views/library/LibraryLocalAlbums'
import LibraryLocalDirectory from './views/library/LibraryLocalDirectory'

global.baseURL = '/'

ReactDOM.render(
	<Provider store={store}>
		<Router history={hashHistory}>
			<Route path={global.baseURL} component={App}>

     			<IndexRoute component={Queue} />
				<Route path="queue" component={Queue} />
				<Route path="settings" component={Settings} />
				<Route path="debug" component={Debug} />
				
				<Route path="search/:query" component={Search} />
				<Route path="search/:query/:type" component={Search} />
				<Route path="album/:uri" component={Album} />
				<Route path="artist/:uri" component={Artist} />
				<Route path="playlist/:uri" component={Playlist} />
				<Route path="user/:uri" component={User} />
	
				<Route path="discover" component={Discover} />
				<Route path="discover/featured" component={DiscoverFeatured} />
				<Route path="discover/categories" component={DiscoverCategories} />
				<Route path="discover/categories/:id" component={DiscoverCategory} />
				<Route path="discover/new-releases" component={DiscoverNewReleases} />

				<Route path="library/artists" component={LibraryArtists} />
				<Route path="library/albums" component={LibraryAlbums} />
				<Route path="library/tracks" component={LibraryTracks} />
				<Route path="library/playlists" component={LibraryPlaylists} />
				<Route path="library/local" component={LibraryLocal} />
				<Route path="library/local/artists" component={LibraryLocalArtists} />
				<Route path="library/local/albums" component={LibraryLocalAlbums} />
				<Route path="library/local/directory/:uri" component={LibraryLocalDirectory} />

			</Route>
		</Router>
	</Provider>,
	document.getElementById('app')
);
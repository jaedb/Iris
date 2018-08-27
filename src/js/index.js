/**
 * Base-level application wrapper
 **/

import React, { PropTypes } from 'react';
import ReactDOM from 'react-dom';
import { Provider } from 'react-redux'
import { createStore } from 'redux'
import { Router, Route, Link, IndexRoute, hashHistory } from 'react-router'

import store from './bootstrap.js';
require('../scss/app.scss');

import App from './App';
import Album from './views/Album';
import Artist from './views/Artist';
import Playlist from './views/Playlist';
import User from './views/User';
import Track from './views/Track';
import Queue from './views/Queue';
import QueueHistory from './views/QueueHistory';
import Debug from './views/Debug';
import Search from './views/Search';
import Settings from './views/Settings';

import DiscoverRecommendations from './views/discover/DiscoverRecommendations';
import DiscoverFeatured from './views/discover/DiscoverFeatured';
import DiscoverCategories from './views/discover/DiscoverCategories';
import DiscoverCategory from './views/discover/DiscoverCategory';
import DiscoverNewReleases from './views/discover/DiscoverNewReleases';

import LibraryArtists from './views/library/LibraryArtists';
import LibraryAlbums from './views/library/LibraryAlbums';
import LibraryTracks from './views/library/LibraryTracks';
import LibraryPlaylists from './views/library/LibraryPlaylists';
import LibraryBrowse from './views/library/LibraryBrowse';

import EditPlaylist from './views/modals/EditPlaylist';
import CreatePlaylist from './views/modals/CreatePlaylist';
import EditRadio from './views/modals/EditRadio';
import AddToQueue from './views/modals/AddToQueue';
import InitialSetup from './views/modals/InitialSetup';
import KioskMode from './views/modals/KioskMode';
import ShareAuthorization from './views/modals/ShareAuthorization';
import AddToPlaylist from './views/modals/AddToPlaylist';
import ImageZoom from './views/modals/ImageZoom';

global.baseURL = '/';

ReactDOM.render(
	<Provider store={store}>
		<Router history={hashHistory}>
			<Route path={global.baseURL} component={App}>

				<IndexRoute component={Queue} />
				 
				<Route path="initial-setup" component={InitialSetup} />
				<Route path="kiosk-mode" component={KioskMode} />
				<Route path="add-to-playlist/:uris" component={AddToPlaylist} />
				<Route path="image-zoom" component={ImageZoom} />

				<Route path="queue" component={Queue} />
				<Route path="queue/history" component={QueueHistory} />
				<Route path="queue/radio" component={EditRadio} />
				<Route path="queue/add-uri" component={AddToQueue} />
				<Route path="settings/debug" component={Debug} />
				<Route path="settings(/service/:sub_view)" component={Settings} />
				<Route path="settings/share-authorization" component={ShareAuthorization} />
				
				<Route path="search(/:type/:term)" component={Search} />
				<Route path="album/:uri" component={Album} />
				<Route path="artist/:uri(/:sub_view)" component={Artist} />
				<Route path="playlist/create" component={CreatePlaylist} />
				<Route path="playlist/:uri" component={Playlist} />
				<Route path="playlist/:uri/edit" component={EditPlaylist} />
				<Route path="user/:uri" component={User} />
				<Route path="track/:uri" component={Track} />
	
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
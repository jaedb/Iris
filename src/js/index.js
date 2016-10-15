/**
 * Base-level application wrapper
 **/

import React, { PropTypes } from 'react';
import ReactDOM from 'react-dom';
import { Provider } from 'react-redux'
import { createStore } from 'redux'
import { Router, Route, Link, hashHistory } from 'react-router'

import store from './bootstrap.js'
import App from './components/App'
import Album from './components/Album'
import Queue from './components/Queue'

ReactDOM.render(
	<Provider store={store}>
		<Router history={hashHistory}>
			<Route path="/" component={App}>
	
				<Route path="album/:uri" component={Album} />
				<Route path="queue" component={Queue} />

			</Route>
		</Router>
	</Provider>,
	document.getElementById('app')
);
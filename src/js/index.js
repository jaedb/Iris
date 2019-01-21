/**
 * Base-level application wrapper
 **/

import React, { PropTypes } from 'react';;
import ReactDOM from 'react-dom';;
import { Provider } from 'react-redux';
import { createStore } from 'redux';
//import { Router, Route, Link, IndexRoute, hashHistory } from 'react-router';
import { BrowserRouter, Route, IndexRoute } from "react-router-dom";

import store from './bootstrap.js';
require('../scss/app.scss');

import App from './App';

global.baseURL = '/';

ReactDOM.render(
	<Provider store={store}>
		<BrowserRouter>
			<Route path="/" component={App} />
		</BrowserRouter>
	</Provider>,
	document.getElementById('app')
);
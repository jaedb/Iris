
import React from 'react';;
import ReactDOM from 'react-dom';;
import { Provider } from 'react-redux';
import { createStore } from 'redux';
import { BrowserRouter, Route, IndexRoute } from "react-router-dom";

import store from './bootstrap.js';
require('../scss/app.scss');

import App from './App';

ReactDOM.render(
	<Provider store={store}>
		<BrowserRouter basename="/iris">
			<Route path="/" component={App} />
		</BrowserRouter>
	</Provider>,
	document.getElementById('app')
);
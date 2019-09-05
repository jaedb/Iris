
import React from 'react';
import ReactDOM from 'react-dom';
import { Provider } from 'react-redux';
import { BrowserRouter, Route, IndexRoute } from 'react-router-dom';

import store from './store/index';
import App from './App';

require('../scss/app.scss');

ReactDOM.render(
  <Provider store={store}>
    <BrowserRouter basename="/iris">
      <Route path="/" component={App} />
    </BrowserRouter>
  </Provider>,
  document.getElementById('app'),
);

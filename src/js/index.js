
import React from 'react';
import ReactDOM from 'react-dom';
import { Provider } from 'react-redux';
import { BrowserRouter, Route } from 'react-router-dom';
import { PersistGate } from 'redux-persist/integration/react';

import { store, persistor } from './store/index';
import App from './App';

require('../scss/app.scss');

ReactDOM.render(
  <Provider store={store}>
    <PersistGate loading={null} persistor={persistor}>
      <BrowserRouter basename="/iris">
        <Route path="/" component={App} />
      </BrowserRouter>
    </PersistGate>
  </Provider>,
  document.getElementById('app'),
);

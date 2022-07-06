import React from 'react';
import { createRoot } from 'react-dom/client';
import { Provider } from 'react-redux';
import { BrowserRouter, Route, Routes } from 'react-router-dom';
import { PersistGate } from 'redux-persist/integration/react';

import { store, persistor } from './store/index';
import App from './App';
import { _ } from 'core-js';

require('../scss/app.scss');

const root = createRoot(document.getElementById('app'));
root.render(
  <Provider store={store}>
    <PersistGate loading={null} persistor={persistor}>
      <BrowserRouter basename="/iris">
        <Routes>
          <Route path="*" element={<App />} />
        </Routes>
      </BrowserRouter>
    </PersistGate>
  </Provider>,
);

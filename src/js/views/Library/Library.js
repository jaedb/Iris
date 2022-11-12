import React from 'react';
import { Route, Routes } from 'react-router-dom';
import Artists from './Artists';
import Albums from './Albums';
import Tracks from './Tracks';
import Playlists from './Playlists';
import Browse from './Browse';
import BrowseDirectory from './BrowseDirectory';

export default () => (
  <Routes>
    <Route
      path="artists/"
      element={<Artists />}
    />
    <Route
      path="albums/"
      element={<Albums />}
    />
    <Route
      path="tracks/"
      element={<Tracks />}
    />
    <Route
      path="playlists/"
      element={<Playlists />}
    />
    <Route
      path="browse/"
      element={<Browse />}
    />
    <Route
      path="browse/:uri/:name"
      element={<BrowseDirectory />}
    />
  </Routes>
);

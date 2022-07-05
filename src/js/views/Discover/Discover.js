import React from 'react';
import { Route, Routes } from 'react-router-dom';
import Recommendations from './Recommendations';
import FeaturedPlaylists from './FeaturedPlaylists';
import NewReleases from './NewReleases';
import Moods from './Moods';
import PlaylistGroup from './PlaylistGroup';
import Playlist from '../Playlist';

export default () => (
  <Routes>
    <Route
      path="recommendations/"
      element={<Recommendations />}
    />
    <Route
      path="recommendations/:uri"
      element={<Recommendations />}
    />
    <Route
      path="moods"
      element={<Moods />}
    />
    <Route
      path="moods/:uri/"
      element={<PlaylistGroup />}
    />
    <Route
      path="moods/:uri/:name"
      element={<PlaylistGroup />}
    />
    <Route
      path="featured-playlists"
      element={<FeaturedPlaylists />}
    />
    <Route
      path="featured-playlists/playlist/:uri/"
      element={<Playlist />}
    />
    <Route
      path="featured-playlists/playlist/:uri/:name/"
      element={<Playlist />}
    />
    <Route
      path="featured-playlists/playlist_group/:uri/"
      element={<PlaylistGroup />}
    />
    <Route
      path="featured-playlists/playlist_group/:uri/:name"
      element={<PlaylistGroup />}
    />
    <Route
      path="new-releases/"
      element={<NewReleases />}
    />
  </Routes>
);

import React from 'react';
import { Route, Switch } from 'react-router-dom';
import Recommendations from './Recommendations';
import FeaturedPlaylists from './FeaturedPlaylists';
import NewReleases from './NewReleases';
import Moods from './Moods';
import PlaylistGroup from './PlaylistGroup';
import Playlist from '../Playlist';

export default () => (
  <Switch>
    <Route
      exact
      path="/discover/recommendations/:uri?"
      component={Recommendations}
    />
    <Route
      exact
      path="/discover/moods"
      component={Moods}
    />
    <Route
      exact
      path="/discover/moods/:uri/:name?"
      component={PlaylistGroup}
    />
    <Route
      exact
      path="/discover/featured-playlists"
      component={FeaturedPlaylists}
    />
    <Route
      exact
      path="/discover/featured-playlists/playlist/:uri/:name?"
      component={Playlist}
    />
    <Route
      exact
      path="/discover/featured-playlists/playlist_group/:uri/:name?"
      component={PlaylistGroup}
    />
    <Route
      exact
      path="/discover/new-releases"
      component={NewReleases}
    />
  </Switch>
);

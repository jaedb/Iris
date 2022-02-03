import React from 'react';
import { Route, Switch } from 'react-router-dom';
import Artists from './Artists';
import Albums from './Albums';
import Tracks from './Tracks';
import Playlists from './Playlists';
import Browse from './Browse';
import BrowseDirectory from './BrowseDirectory';

export default () => (
  <Switch>
    <Route
      exact
      path="/library/artists"
      component={Artists}
    />
    <Route
      exact
      path="/library/albums"
      component={Albums}
    />
    <Route
      exact
      path="/library/tracks"
      component={Tracks}
    />
    <Route
      exact
      path="/library/playlists"
      component={Playlists}
    />
    <Route
      exact
      path="/library/browse"
      component={Browse}
    />
    <Route
      exact
      path="/library/browse/:name/:uri"
      component={BrowseDirectory}
    />
  </Switch>
);

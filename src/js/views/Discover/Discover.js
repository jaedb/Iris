import React from 'react';
import { Route, Switch } from 'react-router-dom';
import Recommendations from './Recommendations';
import Featured from './Featured';
import Categories from './Categories';
import Category from './Category';
import NewReleases from './NewReleases';
import Moods from './Moods';
import PlaylistGroup from './PlaylistGroup';

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
      path="/discover/featured"
      component={Featured}
    />
    <Route
      exact
      path="/discover/categories/:uri"
      component={Category}
    />
    <Route
      exact
      path="/discover/categories"
      component={Categories}
    />
    <Route
      exact
      path="/discover/new-releases"
      component={NewReleases}
    />
  </Switch>
);

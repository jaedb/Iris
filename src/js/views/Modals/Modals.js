import React from 'react';
import { Route, Switch } from 'react-router-dom';
import EditPlaylist from './EditPlaylist';
import CreatePlaylist from './CreatePlaylist';
import EditRadio from './EditRadio';
import AddToQueue from './AddToQueue';
import InitialSetup from './InitialSetup';
import KioskMode from './KioskMode';
import ShareConfig from './ShareConfig';
import ImportConfig from './ImportConfig';
import AddToPlaylist from './AddToPlaylist';
import ImageZoom from './ImageZoom';
import HotkeysInfo from './HotkeysInfo';
import EditCommand from './EditCommand';
import Reset from './Reset';
import Servers from './Servers';

export default () => (
  <Switch>
    <Route path="/modal/initial-setup" component={InitialSetup} />
    <Route path="/modal/kiosk-mode" component={KioskMode} />
    <Route path="/modal/add-to-playlist/:uris" component={AddToPlaylist} />
    <Route path="/modal/image-zoom" component={ImageZoom} />
    <Route path="/modal/hotkeys" component={HotkeysInfo} />
    <Route path="/modal/share-config" component={ShareConfig} />
    <Route path="/modal/import-config/:source?" component={ImportConfig} />
    <Route path="/modal/reset" component={Reset} />
    <Route path="/modal/servers" component={Servers} />
    <Route path="/modal/edit-command/:id?" component={EditCommand} />
    <Route path="/modal/radio" component={EditRadio} />
    <Route path="/modal/add-uri" component={AddToQueue} />
    <Route path="/modal/create-playlist/:uris?" component={CreatePlaylist} />
    <Route path="/modal/edit-playlist/:uri" component={EditPlaylist} />
  </Switch>
);

import React from 'react';
import { Route, Routes } from 'react-router-dom';
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
  <Routes>
    <Route path="initial-setup" element={<InitialSetup />} />
    <Route path="kiosk-mode" element={<KioskMode />} />
    <Route path="add-to-playlist/:uris" element={<AddToPlaylist />} />
    <Route path="image-zoom" element={<ImageZoom />} />
    <Route path="hotkeys" element={<HotkeysInfo />} />
    <Route path="share-config" element={<ShareConfig />} />
    <Route path="import-config/" element={<ImportConfig />} />
    <Route path="import-config/:source" element={<ImportConfig />} />
    <Route path="reset" element={<Reset />} />
    <Route path="servers" element={<Servers />} />
    <Route path="edit-command/" element={<EditCommand />} />
    <Route path="edit-command/:id" element={<EditCommand />} />
    <Route path="radio" element={<EditRadio />} />
    <Route path="add-uri" element={<AddToQueue />} />
    <Route path="create-playlist/" element={<CreatePlaylist />} />
    <Route path="create-playlist/:uris" element={<CreatePlaylist />} />
    <Route path="edit-playlist/:uri" element={<EditPlaylist />} />
  </Routes>
);

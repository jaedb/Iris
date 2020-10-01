import React from 'react';
import ReactGA from 'react-ga';
import localForage from 'localforage';
import { compact } from 'lodash';
import { arrayOf } from '../../util/arrays';
import URILink from '../../components/URILink';
import { uriSource, upgradeSpotifyPlaylistUris, uriType, titleCase } from '../../util/helpers';
import {
  formatTracks,
  formatTrack,
  formatSimpleObject,
  formatAlbum,
  formatArtist,
  formatPlaylist,
  formatUser,
  formatSimpleObjects,
} from '../../util/format';
import { handleException } from './actions';

const coreActions = require('./actions.js');
const uiActions = require('../ui/actions.js');
const mopidyActions = require('../mopidy/actions.js');
const googleActions = require('../google/actions.js');
const spotifyActions = require('../spotify/actions.js');

const CoreMiddleware = (function () {
  return (store) => (next) => (action = {}) => {
    const {
      core,
      ui,
      mopidy,
      spotify,
    } = store.getState();

    window.localForage = localForage;

    // Attach store to window variable. This enables debug tools to directly call on
    // store.getState()
    window._store = store;

    switch (action.type) {
      case 'HANDLE_EXCEPTION':
        const state = store.getState();
        const exported_state = {
          core: { ...state.core },
          ui: { ...state.ui },
          spotify: { ...state.spotify },
          mopidy: { ...state.mopidy },
          pusher: { ...state.pusher },
        };
        const { message } = action;
        let { description } = action;

        // Construct meaningful message and description
        if (!description) {
          if (action.data.xhr && action.data.xhr.responseText) {
            const xhr_response = JSON.parse(action.data.xhr.responseText);
            if (xhr_response.error && xhr_response.error.message) {
              description = xhr_response.error.message;
            }
          } else if (action.data.xhr) {
            description = `${action.data.xhr.status} ${action.data.xhr.statusText}`;
          }
        }


        // Strip out non-essential store info
        delete exported_state.core.albums;
        delete exported_state.core.artists;
        delete exported_state.core.playlists;
        delete exported_state.core.users;
        delete exported_state.core.queue_metadata;
        delete exported_state.core.current_tracklist;
        delete exported_state.spotify.library_albums;
        delete exported_state.spotify.library_artists;
        delete exported_state.spotify.library_playlists;
        delete exported_state.spotify.autocomplete_results;
        delete exported_state.mopidy.library_albums;
        delete exported_state.mopidy.library_artists;
        delete exported_state.mopidy.library_playlists;

        const data = {
          ...action.data,
          message,
          description,
          state: exported_state,
        };

        // Log with Analytics
        if (ui.allow_reporting) {
          ReactGA.event({
            category: 'Error',
            action: message,
            label: description,
            nonInteraction: true,
          });
        }

        if (action.show_notification) {
          store.dispatch(uiActions.createNotification({ content: message, level: 'error', description }));
        }

        console.error(message, description, data);

        break;

      case 'UNSUPPORTED_ACTION':
        store.dispatch(handleException(
          `Action failed (${action.name})`,
          { description: 'Not supported on this type of object' },
        ));
        break;

      case 'PLAY_PLAYLIST':
        if (ui.allow_reporting) {
          ReactGA.event({ category: 'Playlist', action: 'Play', label: action.uri });
        }
        next(action);
        break;

      case 'SAVE_PLAYLIST':
        if (ui.allow_reporting) {
          ReactGA.event({ category: 'Playlist', action: 'Save', label: action.key });
        }
        next(action);
        break;

      case 'CREATE_PLAYLIST':
        if (ui.allow_reporting) {
          ReactGA.event({ category: 'Playlist', action: 'Create', label: +action.name });
        }
        next(action);
        break;

      case 'REORDER_PLAYLIST_TRACKS':
        if (ui.allow_reporting) {
          ReactGA.event({ category: 'Playlist', action: 'Reorder tracks', label: action.key });
        }
        next(action);
        break;

      case 'ADD_PLAYLIST_TRACKS':
        if (ui.allow_reporting) {
          ReactGA.event({ category: 'Playlist', action: 'Add tracks', label: action.playlist_uri });
        }
        next(action);
        break;

      case 'REMOVE_PLAYLIST_TRACKS':
        if (ui.allow_reporting) {
          ReactGA.event({ category: 'Playlist', action: 'Remove tracks', label: action.playlist_uri });
        }
        next(action);
        break;

      case 'DELETE_PLAYLIST':
        if (ui.allow_reporting) {
          ReactGA.event({ category: 'Playlist', action: 'Delete', label: action.uri });
        }
        next(action);
        break;

      case 'SEARCH_STARTED':
        if (ui.allow_reporting) {
          ReactGA.event({ category: 'Search', action: 'Started', label: `${action.type}: ${action.query}` });
        }
        next(action);

        // backends that can handle more than just track results
        // make sure they are available and respect our settings
        const uri_schemes = ui.search_uri_schemes || mopidy.uri_schemes;
        const available_full_uri_schemes = ['local:', 'file:', 'gmusic:'];
        const full_uri_schemes = available_full_uri_schemes.filter(
          (full_uri_scheme) => uri_schemes.indexOf(full_uri_scheme) > -1
        );

        // initiate spotify searching
        if (!action.only_mopidy) {
          if (!ui.search_settings || ui.search_settings.spotify) {
            store.dispatch(spotifyActions.getSearchResults(action.query));
          }
        }

        // backend searching (mopidy)
        store.dispatch(
          mopidyActions.getSearchResults(action.search_type, action.query, 100, full_uri_schemes),
        );

        break;

      case 'RESTART':
        location.reload();
        break;

      /**
           * Playlist manipulation
           * */
      case 'PLAYLIST_TRACKS':
        const playlist_tracks = formatTracks(action.tracks);

        store.dispatch({
          type: 'TRACKS_LOADED',
          tracks: playlist_tracks,
        });

        next({ ...action, tracks_uris: arrayOf('uri', playlist_tracks) });
        break;

      case 'PLAYLIST_TRACKS_ADDED':
        const asset = store.getState().core.playlists[action.key];
        store.dispatch(uiActions.createNotification({
          content: <span>Added {action.tracks_uris.length} tracks to <URILink uri={action.key}>{asset ? asset.name : 'playlist'}</URILink></span>,
        }));
        switch (uriSource(action.key)) {
          case 'spotify':
            store.dispatch(spotifyActions.getPlaylist(action.key));
            break;
          case 'm3u':
            store.dispatch(mopidyActions.getPlaylist(action.key));
            break;
          default:
            break;
        }
        next(action);
        break;

      case 'PLAYLIST_TRACKS_REORDERED':
        var playlists = { ...core.playlists };
        var playlist = { ...playlists[action.key] };
        var tracks_uris = Object.assign([], playlist.tracks_uris);

        // handle insert_before offset if we're moving BENEATH where we're slicing tracks
        var { insert_before } = action;
        if (insert_before > action.range_start) {
          insert_before -= action.range_length;
        }

        // cut our moved tracks into a new array
        var tracks_to_move = tracks_uris.splice(action.range_start, action.range_length);
        tracks_to_move.reverse();

        for (i = 0; i < tracks_to_move.length; i++) {
          tracks_uris.splice(insert_before, 0, tracks_to_move[i]);
        }

        var snapshot_id = null;
        if (action.snapshot_id) {
          snapshot_id = action.snapshot_id;
        }

        // Update our playlist
        playlist.tracks_uris = tracks_uris;
        playlist.snapshot_id = snapshot_id;

        // Trigger normal playlist updating
        store.dispatch({
          type: 'PLAYLISTS_LOADED',
          playlists: [playlist],
        });
        break;

      case 'PLAYLIST_TRACKS_REMOVED':
        var playlists = { ...core.playlists };
        var playlist = { ...playlists[action.key] };
        var tracks_uris = Object.assign([], playlist.tracks_uris);

        var indexes = action.tracks_indexes.reverse();
        for (var i = 0; i < indexes.length; i++) {
          tracks_uris.splice(indexes[i], 1);
        }

        var snapshot_id = null;
        if (action.snapshot_id) {
          snapshot_id = action.snapshot_id;
        }

        // Update our playlist
        playlist.tracks_uris = tracks_uris;
        playlist.snapshot_id = snapshot_id;

        // Trigger normal playlist updating
        store.dispatch({
          type: 'PLAYLISTS_LOADED',
          playlists: [playlist],
        });
        break;

      /**
       * Asset Load commands
       *
       * These are called from views and other middleware to load
       * assets. This is where we can return already indexed records
       * where appropriate
       * */
      case 'LOAD_ITEMS':
        action.uris.forEach((uri) => {
          store.dispatch(uiActions.startLoading(uri));
          store.dispatch({
            type: `LOAD_${uriType(uri).toUpperCase()}`,
            uri,
            options: action.options,
          });
        });
        break;

      case 'LOAD_TRACK': {
        const fetchTrack = () => {
          switch (uriSource(action.uri)) {
            case 'spotify':
              store.dispatch(spotifyActions.getTrack(action.uri, action.options));

              if (spotify.me) {
                store.dispatch(spotifyActions.following(action.uri));
              }
              break;

            default:
              store.dispatch(mopidyActions.getTrack(action.uri, action.options));
              break;
          }
        };

        if (action.options.forceRefetch) {
          console.info(`Force-refetching "${action.uri}"`);
          fetchTrack();
          break;
        }
        if (
          store.getState().core.items[action.uri]
          && store.getState().core.items[action.uri].images
        ) {
          console.info(`${action.uri}" already in index`);
          store.dispatch(uiActions.stopLoading(action.uri));
          break;
        }

        localForage.getItem(action.uri).then((result) => {
          if (result) {
            console.info(`Loading "${action.uri}" from database`);
            store.dispatch(coreActions.restoreItemsFromColdStore([result]));

            if (!result.images) {
              fetchTrack();
            }
          } else {
            fetchTrack();
          }
        });

        next(action);
        break;
      }

      case 'LOAD_ALBUM':
        const fetchAlbum = () => {
          switch (uriSource(action.uri)) {
            case 'spotify':
              store.dispatch(spotifyActions.getAlbum(action.uri, action.options));

              if (spotify.me) {
                store.dispatch(spotifyActions.following(action.uri));
              }
              break;

            default:
              store.dispatch(mopidyActions.getAlbum(action.uri, action.options));
              break;
          };
        };

        if (action.options.forceRefetch) {
          console.info(`Force-refetching "${action.uri}"`);
          fetchAlbum();
          break;
        }
        if (
          store.getState().core.items[action.uri]
          && (!action.options.full || store.getState().core.items[action.uri].tracks)
        ) {
          console.info(`${action.uri}" already in index`);
          store.dispatch(uiActions.stopLoading(action.uri));
          break;
        }

        localForage.getItem(action.uri).then((result) => {
          if (result) {
            console.info(`Loading "${action.uri}" from database`);
            store.dispatch(coreActions.restoreItemsFromColdStore([result]));

            // We don't have the complete Album, so refetch
            if (!result.tracks) {
              fetchAlbum();
            }
          } else {
            fetchAlbum();
          }
        });

        next(action);
        break;

      case 'LOAD_ARTIST':
        const fetchArtist = () => {
          switch (uriSource(action.uri)) {
            case 'spotify':
              store.dispatch(spotifyActions.getArtist(action.uri, action.options));

              if (spotify.me) {
                store.dispatch(spotifyActions.following(action.uri));
              }
              break;

            default:
              store.dispatch(mopidyActions.getArtist(action.uri, action.options));
              break;
          }
        };

        if (action.options.forceRefetch) {
          console.info(`Force-refetching "${action.uri}"`);
          fetchArtist();
          break;
        }
        if (
          store.getState().core.items[action.uri]
          && (
            !action.options.full
            || (
              store.getState().core.items[action.uri].tracks
              && store.getState().core.items[action.uri].albums_uris
              && store.getState().core.items[action.uri].images
            )
          )
        ) {
          console.info(`${action.uri}" already in index`);
          store.dispatch(coreActions.loadItems(store.getState().core.items[action.uri].albums_uris));
          store.dispatch(uiActions.stopLoading(action.uri));
          break;
        }

        localForage.getItem(action.uri).then((artist) => {
          if (artist) {
            console.info(`Restoring "${action.uri}" and ${artist.albums_uris ? artist.albums_uris.length : 0} albums from database`);
            store.dispatch(coreActions.restoreItemsFromColdStore([artist]));

            if (artist.albums_uris) {
              const promises = artist.albums_uris.map((albumUri) => localForage.getItem(albumUri));
              console.time('restoring')
              Promise.all(promises).then(
                (albums) => {
                  store.dispatch(coreActions.restoreItemsFromColdStore(compact(albums)));
                  console.timeEnd('restoring')
                },
              );
            }

            if (action.options.full && (!artist.tracks || !artist.albums_uris || !artist.images)) {
              fetchArtist();
            }
          } else {
            fetchArtist();
          }
        });

        next(action);
        break;

      case 'LOAD_PLAYLIST':
        const fetchPlaylist = () => {
          switch (uriSource(action.uri)) {
            case 'spotify':
              store.dispatch(spotifyActions.getPlaylist(action.uri, action.options));

              if (spotify.me) {
                store.dispatch(spotifyActions.following(action.uri));
              }
              break;

            default:
              store.dispatch(mopidyActions.getPlaylist(action.uri, action.options));
              break;
          }
        };

        if (action.options.forceRefetch) {
          console.info(`Force-refetching "${action.uri}"`);
          fetchPlaylist();
          break;
        }
        if (
          store.getState().core.items[action.uri]
          && (
            !action.options.full
            || store.getState().core.items[action.uri].tracks
          )
        ) {
          console.info(`${action.uri}" already in index`);
          store.dispatch(uiActions.stopLoading(action.uri));
          break;
        }

        localForage.getItem(action.uri).then((result) => {
          if (result) {
            console.info(`Restoring "${action.uri}" from database`);
            store.dispatch(coreActions.restoreItemsFromColdStore([result]));
            if (!action.options.full || !result.tracks) {
              fetchPlaylist();
            }
          } else {
            fetchPlaylist();
          }
        });

        next(action);
        break;

      case 'LOAD_USER':
        if (
          !action.options.forceRefetch
          && store.getState().core.users[action.uri]
          && store.getState().core.users[action.uri].playlists_uris) {
          console.info(`Loading "${action.uri}" from index`);
          break;
        }

        switch (uriSource(action.uri)) {
          case 'spotify':
            store.dispatch(spotifyActions.getUser(action.uri));

            if (spotify.me) {
              store.dispatch(spotifyActions.following(action.uri));
            }
            break;

          default:
            // No Mopidy mechanism for users
            break;
        }

        next(action);
        break;

      case 'LOAD_USER_PLAYLISTS':
        if (
          !action.options.forceRefetch
          && store.getState().core.users[action.uri]
          && store.getState().core.users[action.uri].playlists_uris) {
          console.info(`Loading "${action.uri}" playlists from index`);
          break;
        }

        switch (uriSource(action.uri)) {
          case 'spotify':
            store.dispatch(spotifyActions.getUserPlaylists(action.uri));
            break;

          default:
            // No Mopidy mechanism for users
            break;
        }

        next(action);
        break;

      case 'LOAD_LIBRARY':
        store.dispatch(uiActions.startLoading(action.uri, action.uri));
        const fetchLibrary = () => {
          switch (uriSource(action.uri)) {
            case 'spotify':
              store.dispatch(
                spotifyActions[`getLibrary${titleCase(uriType(action.uri))}`](action.options.forceRefetch),
              );
              break;
            case 'google':
              store.dispatch(
                googleActions[`getLibrary${titleCase(uriType(action.uri))}`](),
              );
              break;
            default:
              store.dispatch(
                mopidyActions[`getLibrary${titleCase(uriType(action.uri))}`](),
              );
              break;
          }
        };

        if (action.options.forceRefetch) {
          console.info(`Force-refetching "${action.uri}"`);
          fetchLibrary();
          break;
        }
        if (store.getState().core.libraries[action.uri]) {
          console.info(`${action.uri}" already in index`);
          store.dispatch(uiActions.stopLoading(action.uri));
          break;
        }

        localForage.getItem(action.uri).then((library) => {
          if (library) {
            console.info(`Restoring "${action.uri}" and ${library.items_uris.length} items from database`);

            const promises = library.items_uris.map((libraryItem) => localForage.getItem(libraryItem));
            Promise.all(promises).then(
              (libraryItems) => {
                store.dispatch(coreActions.restoreItemsFromColdStore(compact(libraryItems)));
                store.dispatch(coreActions.restoreLibraryFromColdStore(library));
              },
            );

          } else {
            fetchLibrary();
          }
        });

        next(action);
        break;

      case 'ADD_TO_LIBRARY': {
        const library = store.getState().core.libraries[action.uri];
        if (library) {
          library.items_uris.push(action.item.uri);
          store.dispatch(coreActions.libraryLoaded(library));
        } else {
          // Clear our stored library. This prevents the next call to possibly restore a stale
          // library listing.
          localForage.removeItem(action.uri);
        }
        store.dispatch(coreActions.itemLoaded({ ...action.item, in_library: true }));
        next(action);
        break;
      }

      case 'REMOVE_FROM_LIBRARY': {
        const library = store.getState().core.libraries[action.uri];
        if (library) {
          const items_uris = library.items_uris.filter((uri) => uri !== action.itemUri);
          store.dispatch(coreActions.libraryLoaded({
            ...library,
            items_uris,
          }));
        }

        store.dispatch(coreActions.removeItem(action.itemUri));
        localForage.removeItem(action.itemUri);

        next(action);
        break;
      }

      /**
       * Index actions
       * These modify our asset indexes, which are used globally
       * */

      case 'CURRENT_TRACK_LOADED': {
        const track = formatTrack(action.track);
        store.dispatch(coreActions.itemLoaded(track));
        next({
          ...action,
          track,
        });
        break;
      }

      case 'QUEUE_LOADED':
        store.dispatch(coreActions.tracksLoaded(action.tracks));
        action.tracks = formatTracks(action.tracks);
        next(action);
        break;

      case 'TRACKS_LOADED':
        var tracks_index = { ...core.tracks };
        var artists_index = core.artists;
        var albums_index = core.albums;
        var tracks_loaded = [];
        var artists_loaded = [];
        var albums_loaded = [];

        for (const raw_track of action.tracks) {
          var track = formatTrack(raw_track);

          if (tracks_index[track.uri] !== undefined) {
            track = { ...tracks_index[track.uri], ...track };
          }

          if (raw_track.album) {
            track.album = formatSimpleObject(raw_track.album);

            if (!albums_index[raw_track.album.uri]) {
              albums_loaded.push(raw_track.album);
            }
          }

          if (raw_track.artists && raw_track.artists.length > 0) {
            track.artists = [];

            for (var artist of raw_track.artists) {
              track.artists.push(formatSimpleObject(artist));

              // Not already in our index, so let's add it
              if (!artists_index[artist.uri]) {
                artists_loaded.push(artist);
              }
            }
          }

          tracks_loaded.push(track);
        }

        action.tracks = tracks_loaded;

        next(action);
        break;

      case 'ALBUMS_LOADED':
        var albums_index = { ...core.albums };
        var albums_loaded = [];
        var artists_loaded = [];
        var tracks_loaded = [];

        for (const raw_album of action.albums) {
          let album = formatAlbum(raw_album);

          if (albums_index[album.uri]) {
            album = { ...albums_index[album.uri], ...album };
          }

          if (raw_album.tracks) {
            album.tracks = raw_album.tracks.map((track) => ({
              ...formatTrack(track),
              album: formatSimpleObject(album)
            }));
          }

          if (raw_album.artists) {
            album.artists = formatSimpleObjects(raw_album.artists);
          }

          albums_loaded.push(album);
        }

        action.albums = albums_loaded;

        if (artists_loaded.length > 0) {
          store.dispatch(coreActions.items(artists_loaded));
        }
        if (tracks_loaded.length > 0) {
          store.dispatch(coreActions.items(tracks_loaded));
        }

        store.dispatch(coreActions.updateColdStore(albums_loaded));

        next(action);
        break;

      case 'ITEMS_LOADED':
        const mergedItems = [];
        action.items.forEach((item) => {
          mergedItems.push({
            ...core.items[item.uri] || {},
            ...item,
          });
        });

        store.dispatch(uiActions.stopLoading(arrayOf('uri', action.items)));
        store.dispatch(coreActions.updateColdStore(mergedItems));
        next({
          ...action,
          items: mergedItems,
        });
        break;

      case 'LIBRARY_LOADED':
        store.dispatch(uiActions.stopLoading(action.library.uri));
        store.dispatch(coreActions.updateColdStore([action.library]));
        next(action);
        break;

      case 'ARTISTS_LOADED':
        var artists_index = { ...core.artists };
        var artists_loaded = [];
        var tracks_loaded = [];

        for (const raw_artist of action.artists) {
          var artist = formatArtist(raw_artist);
          artist = { ...artists_index[artist.uri], ...artist };
          artists_loaded.push(artist);
        }

        store.dispatch(coreActions.updateColdStore(artists_loaded));
        next({
          ...action,
          artists: artists_loaded,
        });
        break;

      case 'PLAYLISTS_LOADED':
        var playlists_index = { ...core.playlists };
        var playlists_loaded = [];
        var tracks_loaded = [];

        for (var playlist of action.playlists) {
          playlist = formatPlaylist(playlist);

          // Detect editability
          switch (uriSource(playlist.uri)) {
            case 'm3u':
              playlist.can_edit = true;
              break;

            case 'spotify':
              if (spotify.authorization && spotify.me) {
                playlist.can_edit = (playlist.owner && playlist.owner.id == spotify.me.id);
              }
          }

          // Already have this playlist partially in our index
          if (playlists_index[playlist.uri]) {
            playlist = { ...playlists_index[playlist.uri], ...playlist };

            // Setup placeholder tracks_uris
            if (playlist.tracks_uris === undefined) {
              playlist.tracks_uris = [];
            }
          }

          // Load our tracks
          if (playlist.tracks) {
            var tracks = formatTracks(playlist.tracks);
            var tracks_uris = arrayOf('uri', tracks);
            playlist.tracks_uris = tracks_uris;
            delete playlist.tracks;
            tracks_loaded = [...tracks_loaded, ...tracks];
          }

          // Update index
          playlists_loaded.push(playlist);
        }

        action.playlists = playlists_loaded;

        if (tracks_loaded.length > 0) {
          store.dispatch(coreActions.tracksLoaded(tracks_loaded));
        }

        next(action);
        break;

      case 'USERS_LOADED':
        var users_index = { ...core.users };
        var users_loaded = [];

        for (let user of action.users) {
          user = formatUser(user);

          if (users_index[user.uri]) {
            user = { ...users_index[user.uri], ...user };
          }

          users_loaded.push(user);
        }

        action.users = users_loaded;

        next(action);
        break;

      case 'USER_PLAYLISTS_LOADED':
        store.dispatch(coreActions.playlistsLoaded(action.playlists));
        next(action);
        break;

      /**
           * Loaded more linked assets
           * Often fired during lazy-loading or async asset grabbing.
           * We link the parent to these indexed records by {type}s_uris
           * */

      case 'LOADED_MORE':
        var parent_type_plural = `${action.parent_type}s`;
        var parent_index = { ...core[`${action.parent_type}s`] };
        var parent = { ...parent_index[action.parent_key] };

        if (action.records_data.items !== undefined) {
          var records = action.records_data.items;
        } else if (action.records_data.tracks !== undefined) {
          var records = action.records_data.tracks;
        } else if (action.records_data.artists !== undefined) {
          var records = action.records_data.artists;
        } else if (action.records_data.albums !== undefined) {
          var records = action.records_data.albums;
        } else if (action.records_data.playlists !== undefined) {
          var records = action.records_data.playlists;
        } else {
          var records = action.records_data;
        }

        // Pre-emptively format tracks
        // Providers give us tracks in all kinds of structures, so this cleans things first
        if (action.records_type == 'track') {
          records = formatTracks(records);
        }

        var records_type_plural = `${action.records_type}s`;
        var records_index = {};
        var records_uris = arrayOf('uri', records);

        // Merge any extra data (eg more_track's albums)
        if (action.extra_data) {
          records = records.map(record => ({ ...record, ...action.extra_data }));
        }

        // If we're a list of playlists, we need to manually filter Spotify's new URI structure
        // Really poor form because they haven't updated it everywhere, yet
        if (action.records_type == 'playlist') {
          records_uris = upgradeSpotifyPlaylistUris(records_uris);
        }

        // Append our parent object's reference to these records
        var uris = records_uris;
        if (parent[`${records_type_plural}_uris`] !== undefined) {
          uris = [...parent[`${records_type_plural}_uris`], ...uris];
        }
        parent[`${records_type_plural}_uris`] = uris;
        if (action.records_data.next !== undefined) {
          parent[`${records_type_plural}_more`] = action.records_data.next;
        }

        // Parent loaded (well, changed)
        var parent_action = {
          type: `${parent_type_plural.toUpperCase()}_LOADED`,
        };
        parent_action[parent_type_plural] = [parent];
        store.dispatch(parent_action);

        // Records loaded
        var records_action = {
          type: `${records_type_plural.toUpperCase()}_LOADED`,
        };
        records_action[records_type_plural] = records;
        store.dispatch(records_action);

        next(action);
        break;

      case 'ADD_PINNED':
        store.dispatch(coreActions.updatePinned([
          ...store.getState().core.pinned,
          formatSimpleObject(action.item),
        ]));
        next(action);
        break;

      case 'REMOVE_PINNED':
        store.dispatch(coreActions.updatePinned(
          store.getState().core.pinned.filter((pinnedItem) => pinnedItem.uri !== action.uri),
        ));
        next(action);
        break;

      case 'UPDATE_PINNED_URI':
        store.dispatch(coreActions.updatePinned(
          store.getState().core.pinned.map((pinnedItem) => ({
            ...pinnedItem,
            ...(pinnedItem.uri === action.oldUri ? { uri: action.newUri } : {}),
          })),
        ));
        next(action);
        break;

      case 'RESTORE_ITEMS_FROM_COLD_STORE':
        store.dispatch(uiActions.stopLoading(arrayOf('uri', action.items)));
        next(action);
        break;

      case 'RESTORE_LIBRARY_FROM_COLD_STORE':
        store.dispatch(uiActions.stopLoading(action.library.uri));
        next(action);
        break;

      // TODO: Relocate this
      case 'UPDATE_COLD_STORE':
        if (action.items) {
          action.items.map((item) => {
            localForage.getItem(item.uri).then((result) => {
              localForage.setItem(item.uri, { ...(result || {}), ...item });
            });
          });
        }
        break;

      // This action is irrelevant to us, pass it on to the next middleware
      default:
        return next(action);
    }
  };
}());

export default CoreMiddleware;

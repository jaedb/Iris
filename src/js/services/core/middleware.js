import React from 'react';
import ReactGA from 'react-ga';
import localForage from 'localforage';
import { compact } from 'lodash';
import { arrayOf } from '../../util/arrays';
import URILink from '../../components/URILink';
import {
  uriSource,
  upgradeSpotifyPlaylistUris,
  uriType,
  titleCase,
} from '../../util/helpers';
import { ensureLoaded } from '../../util/library';
import {
  formatTracks,
  formatTrack,
  formatSimpleObject,
  injectSortId,
} from '../../util/format';
import { handleException } from './actions';
import { I18n } from '../../locale';

const coreActions = require('./actions.js');
const uiActions = require('../ui/actions.js');
const mopidyActions = require('../mopidy/actions.js');
const googleActions = require('../google/actions.js');
const spotifyActions = require('../spotify/actions.js');

const CoreMiddleware = (function () {
  return (store) => (next) => (action = {}) => {
    const {
      core,
      ui: {
        allow_reporting,
        log_actions,
      },
      mopidy,
      spotify,
    } = store.getState();

    window.localForage = localForage;

    // Attach store to window variable. This enables debug tools to directly call on
    // store.getState()
    window._store = store;

    if (log_actions) {
      const ignored_actions = [
        'START_LOADING',
        'STOP_LOADING',
      ];
      if (!ignored_actions.includes(action.type)) {
        console.log(action);
      }
    }

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
        if (allow_reporting) {
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
        if (allow_reporting) {
          ReactGA.event({ category: 'Playlist', action: 'Play', label: action.uri });
        }
        next(action);
        break;

      case 'SAVE_PLAYLIST':
        if (allow_reporting) {
          ReactGA.event({ category: 'Playlist', action: 'Save', label: action.key });
        }
        next(action);
        break;

      case 'CREATE_PLAYLIST':
        if (allow_reporting) {
          ReactGA.event({ category: 'Playlist', action: 'Create', label: +action.name });
        }
        next(action);
        break;

      case 'REORDER_PLAYLIST_TRACKS':
        if (allow_reporting) {
          ReactGA.event({ category: 'Playlist', action: 'Reorder tracks', label: action.key });
        }
        next(action);
        break;

      case 'ADD_PLAYLIST_TRACKS':
        if (allow_reporting) {
          ReactGA.event({ category: 'Playlist', action: 'Add tracks', label: action.playlist_uri });
        }
        next(action);
        break;

      case 'REMOVE_PLAYLIST_TRACKS':
        if (allow_reporting) {
          ReactGA.event({ category: 'Playlist', action: 'Remove tracks', label: action.playlist_uri });
        }
        next(action);
        break;

      case 'DELETE_PLAYLIST':
        if (allow_reporting) {
          ReactGA.event({ category: 'Playlist', action: 'Delete', label: action.uri });
        }
        next(action);
        break;

      case 'START_SEARCH': {
        const { query } = action;
        const {
          ui: {
            allow_reporting,
            uri_schemes_search_enabled = [],
            search_settings,
          },
        } = store.getState();

        if (allow_reporting) {
          ReactGA.event({
            category: 'Search',
            action: 'Started',
            label: `${query.type}: ${query.term}`,
          });
        }

        console.info(`Searching for ${query.type} matching "${query.term}"`);

        // Trigger reducer immediately; this will hose out any previous results
        next(action);

        if (uri_schemes_search_enabled.includes('spotify:')) {
          if (!search_settings || search_settings.spotify) {
            store.dispatch(spotifyActions.getSearchResults(query));
          }
        }
        store.dispatch(mopidyActions.getSearchResults(
          query,
          100,
          uri_schemes_search_enabled.filter((i) => i !== 'spotify:'), // Omit Spotify; handled above
        ));
        break;
      }

      case 'SEARCH_RESULTS_LOADED': {
        const {
          query: {
            term,
            type,
          },
          resultType,
          results,
        } = action;
        const {
          core: {
            search_results: {
              query: {
                term: prevTerm,
                type: prevType,
              } = {},
              ...allResults
            } = {},
          } = {},
        } = store.getState();

        // Add to our existing results, so long as the search term is the same
        const search_results = {
          query: { term, type },
          ...(term === prevTerm && type === prevType ? allResults : {}),
        };

        // Merge our new results with the existing (if any)
        search_results[resultType] = [
          ...(search_results[resultType] || []),
          ...results,
        ];

        next({
          ...action,
          search_results,
        });
        break;
      }

      case 'PLAYLIST_TRACKS_ADDED': {
        const {
          key,
          tracks_uris,
        } = action;
        const {
          core: {
            items: {
              [key]: asset,
            },
          },
        } = store.getState();
        store.dispatch(uiActions.createNotification({
          content: (
            <I18n path="actions.added_tracks_to" count={tracks_uris.length} contentAfter>
              <URILink type="playlist" uri={key}>{asset ? asset.name : 'playlist'}</URILink>
            </I18n>
          ),
        }));

        switch (uriSource(key)) {
          case 'spotify':
            store.dispatch(spotifyActions.getPlaylist(key, {}));
            break;
          case 'm3u':
            store.dispatch(mopidyActions.getPlaylist(key, {}));
            break;
          default:
            break;
        }
        next(action);
        break;
      }

      // This applies our new sort order based on the origional request (rather than a response)
      // This means we don't need to re-fetch the whole playlist after every sort.
      case 'PLAYLIST_TRACKS_REORDERED': {
        const {
          key,
          snapshot_id,
        } = action;
        let { insert_before } = action;
        const playlist = { ...core.items[key] };
        const tracks = Object.assign([], playlist.tracks);

        // handle insert_before offset if we're moving BENEATH where we're slicing tracks
        if (insert_before > action.range_start) {
          insert_before -= action.range_length;
        }

        // cut our moved tracks into a new array
        const tracks_to_move = tracks.splice(action.range_start, action.range_length);
        tracks_to_move.reverse();

        for (let i = 0; i < tracks_to_move.length; i++) {
          tracks.splice(insert_before, 0, tracks_to_move[i]);
        }

        console.log({ action, tracks: injectSortId(tracks) });

        store.dispatch(coreActions.itemLoaded({
          ...playlist,
          tracks: injectSortId(tracks),
          snapshot_id,
        }));
        break;
      }

      case 'PLAYLIST_TRACKS_REMOVED': {
        const {
          key,
          snapshot_id,
        } = action;
        const playlist = { ...core.items[key] };
        const tracks = Object.assign([], playlist.tracks);
        const indexes = action.tracks_indexes.reverse();
        for (let i = 0; i < indexes.length; i++) {
          tracks.splice(indexes[i], 1);
        }

        store.dispatch(coreActions.itemLoaded({
          ...playlist,
          tracks,
          snapshot_id,
        }));
        break;
      }

      /**
       * Asset Load commands
       *
       * These are called from views and other middleware to load assets. This is where we can
       * return already indexed records where appropriate.
       *
       * We prefer for all actions to be called with a URI and type, allowing us to direct the
       * request appropriately. Sometimes we don't know what kind of asset a URI is, in which case
       * we need to LOAD_URI to ascertain this first.
       * */
      case 'LOAD_ITEMS': {
        const { uris = [], options, itemType = 'uri' } = action;

        uris.forEach((uri) => {
          store.dispatch(uiActions.startLoading(uri));
          store.dispatch({
            type: `LOAD_${itemType.toUpperCase()}`,
            uri,
            options,
          });
        });
        break;
      }

      case 'LOAD_URIS': {
        const { uris, options } = action;

        uris.forEach((uri) => {
          const fetch = () => {
            const source = uriSource(uri);
            // We need to pull type from the URI for Spotify as we use specific HTTP endpoints for
            // each asset type, and their URIs facilitate this.
            if (source === 'spotify') {
              store.dispatch(
                coreActions[`load${titleCase(uriType(uri))}`](uri, options),
              );
            } else {
              mopidyActions.getUris([uri], options);
            }
          };
          ensureLoaded({
            store,
            action: { uri, options },
            fetch,
          });
        });
        break;
      }

      case 'LOAD_TRACK': {
        const { uri, options } = action;

        const fetch = () => {
          switch (uriSource(action.uri)) {
            case 'spotify':
              store.dispatch(spotifyActions.getTrack(uri, options));

              if (spotify.me) {
                store.dispatch(spotifyActions.following(uri));
              }
              break;

            default:
              store.dispatch(mopidyActions.getTrack(uri, options));
              break;
          }
        };
        ensureLoaded({
          store,
          action,
          fetch,
          dependents: ['images'],
          fullDependents: options.lyrics ? ['lyrics_results'] : [],
          type: 'track',
        });

        next(action);
        break;
      }

      case 'LOAD_ALBUM': {
        const fetch = () => {
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
        ensureLoaded({
          store,
          action,
          fetch,
          dependents: ['images'],
          fullDependents: ['tracks'],
          type: 'album',
        });
        next(action);
        break;
      }

      case 'LOAD_ARTIST': {
        const fetch = () => {
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
        ensureLoaded({
          store,
          action,
          fetch,
          dependents: ['images'],
          fullDependents: ['tracks', 'albums_uris'],
          type: 'artist',
        });
        next(action);
        break;
      }

      case 'LOAD_PLAYLIST': {
        const fetch = () => {
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
        ensureLoaded({
          store,
          action,
          fetch,
          dependents: ['images'],
          fullDependents: ['tracks'],
          type: 'playlist',
        });
        next(action);
        break;
      }

      case 'LOAD_USER': {
        const fetch = () => {
          switch (uriSource(action.uri)) {
            case 'spotify':
              store.dispatch(spotifyActions.getUser(action.uri, action.options));
              if (spotify.me) {
                store.dispatch(spotifyActions.following(action.uri));
              }
              break;
            default:
              // No mopidy user model
              break;
          }
        };
        ensureLoaded({
          store,
          action,
          fetch,
          fullDependents: ['playlists_uris'],
          type: 'user',
        });
        next(action);
        break;
      }

      case 'LOAD_CATEGORY':
        const fetch = () => {
          switch (uriSource(action.uri)) {
            case 'spotify':
              store.dispatch(spotifyActions.getCategory(action.uri, action.options));
              break;
            default:
              // No mopidy category model
              break;
          }
        };
        ensureLoaded({
          store,
          action,
          fetch,
          fullDependents: ['playlists_uris'],
          type: 'category',
        });

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

      case 'UNLOAD_LIBRARY': {
        localForage.removeItem(action.uri);
        next(action);
        break;
      }

      case 'ADD_TO_LIBRARY': {
        const library = store.getState().core.libraries[action.uri];
        if (library) {
          library.items_uris.push(action.item.uri);
          store.dispatch(coreActions.libraryLoaded(library));
        } else {
          // Clear our stored library. This prevents the next call from possibly restoring a stale
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

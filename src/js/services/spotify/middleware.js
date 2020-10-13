
import ReactGA from 'react-ga';
import { sha256 } from 'js-sha256';

import { arrayOf } from '../../util/arrays';
import { upgradeSpotifyPlaylistUris } from '../../util/helpers';
import {
  formatTracks,
  formatImages,
  formatUser,
  formatAlbums,
} from '../../util/format';
const coreActions = require('../core/actions');
const uiActions = require('../ui/actions');
const spotifyActions = require('./actions');
const pusherActions = require('../pusher/actions');

const SpotifyMiddleware = (function () {
  return (store) => (next) => (action) => {
    const { spotify } = store.getState();

    switch (action.type) {
      case 'SPOTIFY_AUTHORIZATION_GRANTED':
        if (store.getState().ui.allow_reporting) {
          ReactGA.event({ category: 'Spotify', action: 'Authorization granted' });
        }

        // Flush out the previous user's library
        store.dispatch(spotifyActions.flushLibrary());

        next(action);
        break;

      case 'SPOTIFY_AUTHORIZATION_REVOKED':
        if (store.getState().ui.allow_reporting) {
          ReactGA.event({ category: 'Spotify', action: 'Authorization revoked' });
        }

        store.dispatch(uiActions.createNotification({
          content: 'Logout successful',
          description: 'If you have shared your authorization, make sure you revoke your token',
          sticky: true,
          links: [
            {
              url: 'https://www.spotify.com/nz/account/apps/',
              text: 'Authorized apps',
              new_window: true,
            },
          ],
        }));

        next(action);

        // Now dispatch a getMe to get the backend-provided user
        store.dispatch(spotifyActions.getMe());

        // Flush out the previous user's library
        store.dispatch(spotifyActions.flushLibrary());

        break;

      case 'SPOTIFY_IMPORT_AUTHORIZATION':

        // Flush out the previous user's library
        store.dispatch(spotifyActions.flushLibrary());

        // Wait a few moments before we fetch, allowing the import to complete first
        // TODO: Use callbacks for better code accuracy
        setTimeout(() => { store.dispatch(spotifyActions.getMe()); }, 100);

        next(action);
        break;

      case 'SPOTIFY_RECOMMENDATIONS_LOADED':
        if (store.getState().ui.allow_reporting && action.seeds_uris) {
          ReactGA.event({ category: 'Spotify', action: 'Recommendations', label: action.seeds_uris.join(',') });
        }
        next(action);
        break;

      case 'SPOTIFY_USER_LOADED':
        if (store.getState().ui.allow_reporting && action.data) {
                	ReactGA.event({ category: 'User', action: 'Load', label: action.data.uri });
        }
        next(action);
        break;

      case 'SPOTIFY_CREATE_PLAYLIST':
        store.dispatch(spotifyActions.createPlaylist(action.name, action.description, action.is_private, action.is_collaborative));
        break;

      case 'SPOTIFY_REMOVE_PLAYLIST_TRACKS':
        var playlist = { ...store.getState().core.playlists[action.key] };
        store.dispatch(spotifyActions.deleteTracksFromPlaylist(playlist.uri, playlist.snapshot_id, action.tracks_indexes));
        break;


      case 'SPOTIFY_ADD_PLAYLIST_TRACKS':
        store.dispatch(spotifyActions.addTracksToPlaylist(action.key, action.tracks_uris));
        break;

      case 'SPOTIFY_REORDER_PLAYLIST_TRACKS':
        store.dispatch(spotifyActions.reorderPlaylistTracks(action.key, action.range_start, action.range_length, action.insert_before, action.snapshot_id));
        break;

      case 'SPOTIFY_SAVE_PLAYLIST':
        store.dispatch(spotifyActions.savePlaylist(action.key, action.name, action.description, action.is_public, action.is_collaborative, action.image));
        break;

      case 'SPOTIFY_NEW_RELEASES_LOADED': {
        const albums = formatAlbums(action.data.albums.items);
        store.dispatch(coreActions.itemsLoaded(albums));

        next({
          ...action,
          uris: arrayOf('uri', albums),
          more: action.data.albums.next,
          total: action.data.albums.total,
        });
        break;
      }

      case 'SPOTIFY_FLUSH_LIBRARY': {
        store.dispatch(coreActions.unloadLibrary('spotify:library:artists'));
        store.dispatch(coreActions.unloadLibrary('spotify:library:albums'));
        store.dispatch(coreActions.unloadLibrary('spotify:library:playlists'));
        store.dispatch(coreActions.unloadLibrary('spotify:library:tracks'));
        break;
      }

      case 'SPOTIFY_CATEGORIES_LOADED':
        var categories_index = { ...spotify.categories };
        var categories_loaded = [];

        for (const raw_category of action.categories) {
          let category = { ...raw_category };

          if (!category.uri) {
            category.uri = `category:${category.id}`;
          }

          if (categories_index[category.uri] !== undefined) {
            category = { ...categories_index[category.uri], ...category };
          }

          if (category.icons) {
            category.icons = formatImages(category.icons);
          }

          categories_loaded.push(category);
        }

        action.categories = categories_loaded;
        next(action);
        break;

      case 'SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR':
        const playlistProcessor = store.getState().ui.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR || {};
        switch (playlistProcessor.status) {
          case 'cancelling':
            store.dispatch(uiActions.processCancelled('SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR'));
            break;

          case 'cancelled':
            break;

          default:
            store.dispatch(spotifyActions.getLibraryPlaylistsProcessor(action.data));
            break;
        }
        break;

      case 'SPOTIFY_LIBRARY_PLAYLISTS_LOADED':
        var playlists = [];
        for (var playlist of action.playlists) {
          Object.assign(
            playlist,
            {
              uri: playlist.uri.replace(/spotify:user:([^:]*?):/i, 'spotify:'),
              can_edit: (store.getState().spotify.me && store.getState().spotify.me.id == playlist.owner.id),
              source: 'spotify',
              in_library: true, // assumed because we asked for library items
              tracks_total: playlist.tracks.total,
            },
          );

          // remove our tracklist. It'll overwrite any full records otherwise
          delete playlist.tracks;

          playlists.push(playlist);
        }

        store.dispatch({
          type: 'PLAYLISTS_LOADED',
          playlists,
        });

        // Append our action with the uris. This gets handed down to subsequent middleware and our reducer.
        action.uris = arrayOf('uri', playlists);
        next(action);
        break;

      case 'SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR':
        const artistsProcessor = store.getState().ui.processes.SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR || {};
        switch (artistsProcessor.status) {
          case 'cancelling':
            store.dispatch(uiActions.processCancelled('SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR'));
            break;

          case 'cancelled':
            break;

          default:
            store.dispatch(spotifyActions.getLibraryArtistsProcessor(action.data));
            break;
        }
        break;

      case 'SPOTIFY_LIBRARY_ARTISTS_LOADED':
        var artists = [];
        for (var i = 0; i < action.artists.length; i++) {
          artists.push(
            {

              ...action.artists[i],
              source: 'spotify',
              in_library: true, // assumed because we asked for library items

            },
          );
        }
        store.dispatch({
          type: 'ARTISTS_LOADED',
          artists,
        });

        // Append our action with the uris. This gets handed down to subsequent middleware and our reducer.
        action.uris = arrayOf('uri', artists);
        next(action);
        break;

      case 'SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR':
        const albumsProcessor = store.getState().ui.processes.SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR || {};
        switch (albumsProcessor.status) {
          case 'cancelling':
            store.dispatch(uiActions.processCancelled('SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR'));
            break;

          case 'cancelled':
            break;

          default:
            store.dispatch(spotifyActions.getLibraryAlbumsProcessor(action.data));
            break;
        }
        break;

      case 'SPOTIFY_GET_LIBRARY_TRACKS_AND_PLAY_PROCESSOR':
        store.dispatch(spotifyActions.getLibraryTracksAndPlayProcessor(action.data));
        break;

      case 'SPOTIFY_GET_ALL_PLAYLIST_TRACKS_PROCESSOR':
        store.dispatch(spotifyActions.getAllPlaylistTracksProcessor(action.data));
        break;

      case 'SPOTIFY_LIBRARY_ALBUMS_LOADED':
        var albums = [];
        for (var i = 0; i < action.albums.length; i++) {
          albums.push(
            {

              ...action.albums[i].album,
              in_library: true, // assumed because we asked for library items
              source: 'spotify',
              added_at: action.albums[i].added_at,
              tracks: action.albums[i].album.tracks.items,
              tracks_more: action.albums[i].album.tracks.next,
              tracks_total: action.albums[i].album.tracks.total,
            },
          );
        }

        store.dispatch({
          type: 'ALBUMS_LOADED',
          albums,
        });

        // Append our action with the uris. This gets handed down to subsequent middleware and our reducer.
        action.uris = arrayOf('uri', albums);
        next(action);
        break;

      case 'SPOTIFY_FAVORITES_LOADED':
        if (action.artists.length > 0) {
          store.dispatch({
            type: 'ARTISTS_LOADED',
            artists: action.artists,
          });
          action.artists_uris = arrayOf('uri', action.artists);
        }
        if (action.tracks.length > 0) {
          store.dispatch({
            type: 'TRACKS_LOADED',
            tracks: action.tracks,
          });
          action.tracks_uris = arrayOf('uri', action.tracks);
        }
        next(action);
        break;

      case 'SPOTIFY_LIBRARY_TRACKS_LOADED':
      case 'SPOTIFY_LIBRARY_TRACKS_LOADED_MORE':
        if (action.data) {
          store.dispatch({
            type: 'TRACKS_LOADED',
            tracks: action.data.items,
          });
        }
        next(action);
        break;

      case 'SPOTIFY_TRACK_LOADED':
        store.dispatch({
          type: 'TRACKS_LOADED',
          tracks: [action.data],
        });
        next(action);
        break;

      case 'SPOTIFY_ME_LOADED':
        var me = { ...formatUser(action.me) };

        // We are Anonymous currently so use 'me' name as my Pusher username
        if (store.getState().pusher.username == 'Anonymous') {
          store.dispatch(pusherActions.setUsername(me.name));
        }

        if (store.getState().ui.allow_reporting) {
	                const hashed_username = sha256(me.id);
	                ReactGA.set({ userId: hashed_username });
	                ReactGA.event({ category: 'Spotify', action: 'Authorization verified', label: hashed_username });
	            }

        store.dispatch(coreActions.userLoaded(me));
        action.me = me;
        next(action);
        break;

        // This action is irrelevant to us, pass it on to the next middleware
      default:
        return next(action);
    }
  };
}());

export default SpotifyMiddleware;

import ReactGA from 'react-ga';
import Mopidy from 'mopidy';
import { sha256 } from 'js-sha256';
import { sampleSize, compact, chunk, find } from 'lodash';
import { i18n } from '../../locale';
import {
  generateGuid,
  uriSource,
  setFavicon,
  titleCase,
} from '../../util/helpers';
import {
  digestMopidyImages,
  formatImages,
  formatAlbum,
  formatTrack,
  formatTracks,
  formatPlaylistGroup,
  getTrackIcon,
  formatArtists,
  formatArtist,
  formatPlaylist,
  injectSortId,
  formatSimpleObject,
} from '../../util/format';
import {
  arrayOf,
  shuffle,
  applyFilter,
  sortItems,
  indexToArray,
} from '../../util/arrays';
import { getProvider, getSortSelector } from '../../util/selectors';

const mopidyActions = require('./actions.js');
const coreActions = require('../core/actions.js');
const uiActions = require('../ui/actions.js');
const spotifyActions = require('../spotify/actions.js');
const pusherActions = require('../pusher/actions.js');
const googleActions = require('../google/actions.js');
const lastfmActions = require('../lastfm/actions.js');
const geniusActions = require('../genius/actions.js');
const discogsActions = require('../discogs/actions.js');

/**
 * Fetch the method of our Mopidy API that is being called, by string
 *
 * @param {String} call
 * @param {Object} socket
 */
const getController = (call, socket) => {
  const callParts = call.split('.');
  const model = callParts[0];
  const method = callParts[1];

  let controller = null;
  if (socket && socket[model]) {
    if (method in socket[model] && socket[model][method]) {
      controller = socket[model][method];
    } else {
      controller = socket[model];
    }
  }

  return controller;
};

const MopidyMiddleware = (function () {
  // container for the actual Mopidy socket
  let socket = null;

  // play position timer
  let progress_interval = null;

  // handle all manner of socket messages
  const handleMessage = (ws, store, type, data) => {
    // if debug enabled
    if (store.getState().ui.log_mopidy) {
      console.log('Mopidy', type, data);
    }

    switch (type) {
      case 'state:online':
        store.dispatch({ type: 'MOPIDY_CONNECTED' });

        store.dispatch(mopidyActions.getCurrentTrack());
        store.dispatch(mopidyActions.getPlayState());
        store.dispatch(mopidyActions.getVolume());
        store.dispatch(mopidyActions.getMute());
        store.dispatch(mopidyActions.getConsume());
        store.dispatch(mopidyActions.getRandom());
        store.dispatch(mopidyActions.getRepeat());
        store.dispatch(mopidyActions.getQueue());
        store.dispatch(mopidyActions.getTimePosition());
        store.dispatch(mopidyActions.getUriSchemes());
        store.dispatch(mopidyActions.getStreamTitle());

        // Every 10s update our play position (when playing)
        progress_interval = setInterval(() => {
          if (
            store.getState().mopidy.play_state === 'playing'
            && store.getState().ui.window_focus === true
          ) {
            store.dispatch(mopidyActions.getTimePosition());
          }
        }, 10000);

        break;

      case 'state:offline':
        store.dispatch({ type: 'MOPIDY_DISCONNECTED' });
        store.dispatch(mopidyActions.clearCurrentTrack());

        // reset our playback interval timer
        clearInterval(progress_interval);
        break;

      case 'event:tracklistChanged':
        store.dispatch(mopidyActions.getQueue());

        // Wait a jiffy before we get the next track
        // We don't want to impede snappyness for this luxury request
        setTimeout(
          () => {
            store.dispatch(mopidyActions.getNextTrack());
          },
          1000,
        );
        break;

      case 'event:playbackStateChanged':
        store.dispatch({
          type: 'MOPIDY_PLAY_STATE',
          play_state: data.new_state,
        });
        store.dispatch(mopidyActions.getTimePosition());
        break;

      case 'event:seeked':
        store.dispatch({
          type: 'MOPIDY_TIME_POSITION',
          time_position: data.time_position,
        });
        break;

      case 'event:trackPlaybackEnded':
        store.dispatch(mopidyActions.clearCurrentTrack());
        store.dispatch({
          type: 'MOPIDY_TIME_POSITION',
          time_position: 0,
        });
        break;

      case 'event:trackPlaybackStarted':
        if (data.tl_track) {
          store.dispatch(mopidyActions.currentTrackLoaded(data.tl_track));

          // Wait a jiffy before we get the next track
          // We don't want to impede snappyness for this luxury request
          setTimeout(
            () => {
              store.dispatch(mopidyActions.getNextTrack());
            },
            1000,
          );
        }
        break;

      case 'event:volumeChanged':
        store.dispatch({ type: 'MOPIDY_VOLUME', volume: data.volume });
        break;

      case 'event:muteChanged':
        store.dispatch({ type: 'MOPIDY_MUTE', mute: data.mute });
        break;

      case 'event:optionsChanged':
        store.dispatch(mopidyActions.getConsume());
        store.dispatch(mopidyActions.getRandom());
        store.dispatch(mopidyActions.getRepeat());
        break;

      case 'event:streamTitleChanged':
        store.dispatch(coreActions.streamTitleChanged(data.title));
        break;

      default:
      // console.log('MopidyService: Unhandled message', type, message );
    }
  };

  /**
   * Call something with Mopidy
   *
   * Sends request to Mopidy server, and updates our local storage on return
   * @param string model = which Mopidy model (playback, tracklist, etc)
   * @param string property = TlTracks, Consume, etc
   * @param string value (optional) = value of the property to pass
   * @return promise
   * */
  const request = (store, call, value = {}) => new Promise((resolve, reject) => {
    const loaderId = generateGuid();
    const loaderKey = `mopidy_${call}_${value?.uri || ''}_${value?.uris?.join('_') || ''}`;
    store.dispatch(uiActions.startLoading(loaderId, loaderKey));

    const doRequest = () => {
      const controller = getController(call, socket);

      if (controller) {
        const timeout = setTimeout(
          () => {
            store.dispatch(uiActions.stopLoading(loaderId));
            reject(new Error('Request timed out'));
          },
          30000,
        );

        controller(value)
          .then(
            (response) => {
              clearTimeout(timeout);
              store.dispatch(uiActions.stopLoading(loaderId));
              resolve(response);
            },
            (error) => {
              clearTimeout(timeout);
              store.dispatch(uiActions.stopLoading(loaderId));
              reject(error);
            },
          );
      } else {
        store.dispatch(uiActions.stopLoading(loaderId));
        console.warn(
          'Mopidy request aborted. Either Mopidy is not connected or the request method is invalid. Check the request and your server settings.',
          {
            call, value, socket, controller,
          },
        );
      }
    };

    // Give a 5-second leeway for allowing Mopidy to connect, if it isn't already connected
    if (socket && store.getState().mopidy.connected) {
      doRequest();
    } else {
      console.info('Mopidy not yet connected, waiting 2 seconds');
      setTimeout(
        () => doRequest(),
        2000,
      );
    }
  });

  /**
   * Process our search queries queue. We process one item in the queue and then
   * reiterate if there are any items remaining.
   *
   * @param {Object} store
   * @param {Array} queries
   */
  const processSearchQueue = (store, queue) => {
    const {
      type,
      term,
      requestType,
      uri_scheme,
      method = 'library.search',
      data,
    } = queue.shift();
    const processKey = 'MOPIDY_GET_SEARCH_RESULTS';
    const processor = store.getState().ui.processes[processKey];

    if (processor && processor.status === 'cancelling') {
      store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS'));
      return;
    }

    store.dispatch(uiActions.updateProcess(
      processKey,
      {
        content: i18n(
          'services.mopidy.searching',
          {
            provider: titleCase(uri_scheme.replace(':', '')),
            type: requestType,
          },
        ),
        remaining: queue.length,
      },
    ));

    // Each type has a different method of formatting and destructuring.
    const processResults = {
      albums: (response) => {
        const result = response[0];
        const albums = {};
        if (result.albums) {
          result.albums.forEach((album) => {
            if (album.uri) {
              albums[album.uri] = {
                ...albums[album.uri],
                ...formatAlbum(album),
              };
            }
          });
        }
        if (result.tracks) {
          result.tracks.forEach((track) => {
            if (track.album && track.album.uri) {
              albums[track.album.uri] = {
                ...albums[track.album.uri],
                ...formatAlbum(track.album),
              };
            }
          });
        }
        return indexToArray(albums);
      },
      artists: (response) => {
        const result = response[0];
        const artists = {};
        if (result.artists) {
          result.artists.forEach((artist) => {
            if (artist.uri) {
              artists[artist.uri] = {
                ...artists[artist.uri],
                ...formatArtist(artist),
              };
            }
          });
        }
        if (result.tracks) {
          result.tracks.forEach((track) => {
            if (track.artists) {
              track.artists.forEach((artist) => {
                if (artist && artist.uri) {
                  artists[artist.uri] = {
                    ...artists[artist.uri],
                    ...formatArtist(artist),
                  };
                }
              });
            }
          });
        }
        return indexToArray(artists);
      },
      playlists: (response) => {
        const playlists = response.filter(
          (item) => {
            if (!item.uri.includes(uri_scheme)) return false;
            return item.name.toLowerCase().includes(term.toLowerCase());
          },
        );
        return playlists.map((playlist) => ({
          ...formatPlaylist(playlist),
          uri: playlist.uri,
        }));
      },
      tracks: (response) => {
        const { tracks = [] } = response[0];
        return formatTracks(tracks);
      },
    };

    request(
      store,
      method,
      data,
    ).then(
      (response) => {
        if (response.length > 0) {
          store.dispatch(coreActions.searchResultsLoaded(
            { term, type },
            requestType,
            processResults[requestType](response),
          ));
        }

        if (queue.length) {
          processSearchQueue(store, queue);
        } else {
          store.dispatch(uiActions.processFinished(processKey));
        }
      },
    );
  };

  /**
   * Middleware
   *
   * This behaves like an action interceptor. We listen for specific actions
   * and handle special functionality. If the action is not in our switch, then
   * it just proceeds to the next middleware, or default functionality
   * */
  return (store) => (next) => (action) => {
    switch (action.type) {
      case 'MOPIDY_CONNECT': {
        if (socket != null) socket.close();

        store.dispatch({ type: 'MOPIDY_CONNECTING' });
        const { ssl, host, port } = store.getState().mopidy;
        try {
          socket = new Mopidy({
            webSocketUrl: `ws${ssl ? 's' : ''}://${host}:${port}/mopidy/ws/`,
            callingConvention: 'by-position-or-by-name',
          });
        } catch (exception) {
          console.error(exception);
          break;
        }

        socket.on('state:online', (data) => handleMessage(socket, store, 'state:online', data));
        socket.on('state:offline', (data) => handleMessage(socket, store, 'state:offline', data));
        socket.on('event:tracklistChanged', (data) => handleMessage(socket, store, 'event:tracklistChanged', data));
        socket.on('event:playbackStateChanged', (data) => handleMessage(socket, store, 'event:playbackStateChanged', data));
        socket.on('event:seeked', (data) => handleMessage(socket, store, 'event:seeked', data));
        socket.on('event:trackPlaybackEnded', (data) => handleMessage(socket, store, 'event:trackPlaybackEnded', data));
        socket.on('event:trackPlaybackStarted', (data) => handleMessage(socket, store, 'event:trackPlaybackStarted', data));
        socket.on('event:volumeChanged', (data) => handleMessage(socket, store, 'event:volumeChanged', data));
        socket.on('event:muteChanged', (data) => handleMessage(socket, store, 'event:muteChanged', data));
        socket.on('event:optionsChanged', (data) => handleMessage(socket, store, 'event:optionsChanged', data));
        socket.on('event:streamTitleChanged', (data) => handleMessage(socket, store, 'event:streamTitleChanged', data));

        break;
      }

      case 'MOPIDY_CONNECTED':
        if (store.getState().ui.allow_reporting) {
          ReactGA.event({
            category: 'Mopidy',
            action: 'Connected',
            label: sha256(window.location.hostname),
          });
        }
        next(action);
        break;

      case 'MOPIDY_DISCONNECT':
        if (socket != null) socket.close();
        socket = null;
        store.dispatch({ type: 'MOPIDY_DISCONNECTED' });
        break;

      case 'MOPIDY_DISCONNECTED':
        setFavicon('favicon_error.png');
        break;

      case 'MOPIDY_DEBUG':
        request(store, action.call, action.value)
          .then((response) => {
            store.dispatch({ type: 'DEBUG', response });
          });
        break;

      case 'MOPIDY_UPDATE_SERVER':
        const { servers } = store.getState().mopidy;
        const server = {
          ...servers[action.server.id] || {},
          ...action.server,
        };
        server.url = `http${server.ssl ? 's' : ''}://${server.host}:${server.port}`;
        store.dispatch(mopidyActions.updateServers({ ...servers, [server.id]: server }));
        break;

      case 'MOPIDY_SET_CURRENT_SERVER': {
        const { server } = action;
        const { servers } = store.getState().mopidy;
        let existingServer = null;
        if (action.server.id) {
          existingServer = servers[action.server.id] || undefined;
        } else {
          // No ID provided, see if we have a server setup with the same details already
          existingServer = find(servers, (s) => s.url === server.url);
        }

        if (!existingServer) {
          const create = mopidyActions.addServer(server);
          store.dispatch(create);
          existingServer = create.server;
        }

        store.dispatch(mopidyActions.set({
          current_server: server.id || existingServer?.id,
          host: server.host,
          port: server.port,
          ssl: server.ssl,
          connected: false,
          connecting: false,
        }));

        // Wait a moment for store to update, then attempt connection
        setTimeout(
          () => {
            store.dispatch(mopidyActions.connect());
            store.dispatch(pusherActions.connect());
          },
          250,
        );
        next(action);
        break;
      }

      case 'MOPIDY_GET_SERVER_STATE': {
        let server = { ...store.getState().mopidy.servers[action.id] };
        if (!server.host || !server.port || server.host === '' || server.port === '') break;

        const host = `http${server.ssl ? 's' : ''}://${server.host}:${server.port}`;
        fetch(`${host}/iris/http/get_server_state`)
          .then((response) => response.json())
          .then(({ result }) => {
            server = { ...server, ...result };
            if (result.current_track) {
              const images = result.current_track.images
                ? formatImages(digestMopidyImages(server, result.current_track.images))
                : null;
              server.current_track = formatTrack({ ...result.current_track, images });
            }
            store.dispatch(mopidyActions.updateServer(server));
          })
          .catch((error) => {
            store.dispatch(coreActions.handleException('Could not fetch server', error, host, false));
          });

        break;
      }

      case 'MOPIDY_REMOVE_SERVER': {
        const servers = { ...store.getState().mopidy.servers };
        delete servers[action.id];
        store.dispatch(mopidyActions.updateServers(servers));
        break;
      }

      case 'SET_WINDOW_FOCUS':
        /**
         * Disabled because it causes so many requests without any conscious user input.
         * Will monitor to see if it has any adverse impact.
         */

        /*
        // Focus has just been regained
        if (action.window_focus === true) {
          store.dispatch(mopidyActions.getCurrentTrack());
          store.dispatch(mopidyActions.getStreamTitle());
          store.dispatch(mopidyActions.getPlayState());
          store.dispatch(mopidyActions.getVolume());
          store.dispatch(mopidyActions.getMute());
          store.dispatch(mopidyActions.getConsume());
          store.dispatch(mopidyActions.getRandom());
          store.dispatch(mopidyActions.getRepeat());
          store.dispatch(mopidyActions.getTimePosition());
        }
        */
        next(action);
        break;

      case 'MOPIDY_REQUEST':
        request(store, action.method, action.params)
          .then(
            (response) => {
              if (action.response_callback) {
                action.response_callback(response);
              }
            },
            (error) => {
              if (action.error_callback) {
                action.error_callback(error);
              } else {
                store.dispatch(coreActions.handleException(
                  'Mopidy request failed',
                  error,
                  action.method,
                  action,
                ));
              }
            },
          );
        break;

        /**
       * General playback
       * */

      case 'MOPIDY_PLAY_STATE':
        store.dispatch(uiActions.setWindowTitle(null, action.play_state));

        if (action.play_state == 'playing') {
          setFavicon('favicon.png');
        } else {
          setFavicon('favicon_paused.png');
        }
        next(action);
        break;

      case 'MOPIDY_GET_PLAY_STATE':
        request(store, 'playback.getState')
          .then(
            (response) => {
              store.dispatch({
                type: 'MOPIDY_PLAY_STATE',
                play_state: response,
              });
            },
          );
        break;

      case 'MOPIDY_PLAY':
        request(store, 'playback.play')
          .then(
            (response) => {
              store.dispatch({
                type: 'MOPIDY_PLAY_STATE',
                play_state: 'playing',
              });
            },
          );

        store.dispatch(pusherActions.deliverBroadcast(
          'notification',
          {
            notification: {
              content: `${store.getState().pusher.username + (store.getState().mopidy.play_state == 'paused' ? ' resumed' : ' started')} playback`,
              icon: (store.getState().core.current_track ? getTrackIcon(store.getState().core.current_track, store.getState().core) : false),
            },
          },
        ));
        break;

      case 'MOPIDY_PAUSE':
        request(store, 'playback.pause')
          .then((response) => {
            store.dispatch({
              type: 'MOPIDY_PLAY_STATE',
              play_state: 'paused',
            });
            store.dispatch(pusherActions.deliverBroadcast(
              'notification',
              {
                notification: {
                  content: `${store.getState().pusher.username} paused playback`,
                  icon: (store.getState().core.current_track ? getTrackIcon(store.getState().core.current_track, store.getState().core) : false),
                },
              },
            ));
          });

        break;

      case 'MOPIDY_PREVIOUS':

        // Let the UI know we're in transition
        store.dispatch(uiActions.setCurrentTrackTransition('previous'));

        request(store, 'playback.previous');
        break;

      case 'MOPIDY_NEXT':

        // Let the UI know we're in transition
        store.dispatch(uiActions.setCurrentTrackTransition('next'));

        request(store, 'playback.next')
          .then((response) => {
            store.dispatch(pusherActions.deliverBroadcast(
              'notification',
              {
                notification: {
                  content: `${store.getState().pusher.username} skipped "${store.getState().core.current_track.name}"`,
                  icon: (store.getState().core.current_track ? getTrackIcon(store.getState().core.current_track, store.getState().core) : false),
                },
              },
            ));
          });

        break;

      case 'MOPIDY_STOP':
        request(store, 'playback.stop')
          .then((response) => {
            store.dispatch(mopidyActions.clearCurrentTrack());

            store.dispatch(pusherActions.deliverBroadcast(
              'notification',
              {
                notification: {
                  content: `${store.getState().pusher.username} stopped playback`,
                  icon: (store.getState().core.current_track ? getTrackIcon(store.getState().core.current_track, store.getState().core) : false),
                },
              },
            ));
          });
        break;

      case 'MOPIDY_CHANGE_TRACK':
        request(store, 'playback.play', { tlid: action.tlid })
          .then((response) => {
            store.dispatch(pusherActions.deliverBroadcast(
              'notification',
              {
                notification: {
                  content: `${store.getState().pusher.username} changed track`,
                },
              },
            ));
          });
        break;

      case 'MOPIDY_REMOVE_TRACKS':
        request(store, 'tracklist.remove', { criteria: { tlid: action.tlids } })
          .then((response) => {
            store.dispatch(pusherActions.deliverBroadcast(
              'notification',
              {
                notification: {
                  content: `${store.getState().pusher.username} removed ${action.tlids.length} tracks`,
                },
              },
            ));
          });
        break;

      case 'MOPIDY_GET_REPEAT':
        request(store, 'tracklist.getRepeat')
          .then((response) => {
            store.dispatch({
              type: 'MOPIDY_REPEAT',
              repeat: response,
            });
          });
        break;

      case 'MOPIDY_SET_REPEAT':
        request(store, 'tracklist.setRepeat', [action.repeat]);
        break;

      case 'MOPIDY_GET_RANDOM':
        request(store, 'tracklist.getRandom')
          .then((response) => {
            store.dispatch({
              type: 'MOPIDY_RANDOM',
              random: response,
            });
          });
        break;

      case 'MOPIDY_SET_RANDOM':
        request(store, 'tracklist.setRandom', [action.random]);
        break;

      case 'MOPIDY_GET_CONSUME':
        request(store, 'tracklist.getConsume')
          .then((response) => {
            store.dispatch({
              type: 'MOPIDY_CONSUME',
              consume: response,
            });
          });
        break;

      case 'MOPIDY_SET_CONSUME':
        request(store, 'tracklist.setConsume', [action.consume]);
        break;

      case 'MOPIDY_GET_MUTE':
        request(store, 'mixer.getMute')
          .then((response) => {
            store.dispatch({
              type: 'MOPIDY_MUTE',
              mute: response,
            });
          });
        break;

      case 'MOPIDY_SET_MUTE':
        request(store, 'mixer.setMute', [action.mute])
          .then((response) => {
            store.dispatch(pusherActions.deliverBroadcast(
              'notification',
              {
                notification: {
                  content: `${store.getState().pusher.username + (action.mute ? ' muted' : ' unmuted')} playback`,
                },
              },
            ));
          });
        break;

      case 'MOPIDY_GET_VOLUME':
        request(store, 'mixer.getVolume')
          .then(
            (response) => {
              store.dispatch({
                type: 'MOPIDY_VOLUME',
                volume: response,
              });
            },
          );
        break;

      case 'MOPIDY_SET_VOLUME':
        request(store, 'mixer.setVolume', { volume: action.volume })
          .then(
            (response) => {
              store.dispatch({
                type: 'MOPIDY_VOLUME',
                volume: action.volume,
              });
            },
          );
        break;

      case 'MOPIDY_SET_TIME_POSITION':
        request(store, 'playback.seek', { time_position: action.time_position })
          .then(
            (response) => {
              store.dispatch({
                type: 'MOPIDY_TIME_POSITION',
                time_position: action.time_position,
              });
            },
          );
        break;

      case 'MOPIDY_GET_TIME_POSITION':
        request(store, 'playback.getTimePosition')
          .then(
            (response) => {
              store.dispatch({
                type: 'MOPIDY_TIME_POSITION',
                time_position: response,
              });
            },
          );
        break;

      case 'MOPIDY_GET_URI_SCHEMES':
        request(store, 'getUriSchemes')
          .then(
            (response) => {
              // Remove unsupported schemes
              const remove = ['http', 'https', 'mms', 'rtmp', 'rtmps', 'rtsp', 'sc', 'yt'];
              let uri_schemes = response.filter((uri) => remove.indexOf(uri) === -1);

              // Append with ':' to make them a mopidy URI
              uri_schemes = uri_schemes.map((uri) => `${uri}:`);

              // Enable Iris providers when the backend is available
              store.dispatch(spotifyActions.set({ enabled: uri_schemes.includes('spotify:') }));
              store.dispatch(googleActions.set({ enabled: uri_schemes.includes('gmusic:') }));

              // If we haven't customised our search schemes, add all to search
              if (store.getState().ui.uri_schemes_search_enabled === undefined) {
                store.dispatch(uiActions.set({ uri_schemes_search_enabled: uri_schemes }));
              }

              store.dispatch({ type: 'MOPIDY_URI_SCHEMES', uri_schemes });
            },
          );
        break;

      case 'MOPIDY_PLAY_PLAYLIST': {
        console.debug(action)
        const playlist = store.getState().core.items[action.uri];
        const { sortField, sortReverse } = getSortSelector(store.getState(), 'playlist_tracks');
        if (playlist && playlist.tracks) {
          store.dispatch(
            mopidyActions.playURIs({
              uris: arrayOf(
                'uri',
                sortItems(
                  playlist.tracks.filter((t) => t?.is_playable !== false),
                  sortField,
                  sortReverse,
                ),
              ),
              from: {
                uri: playlist?.uri,
                name: playlist?.name,
                type: 'playlist',
              },
              ...action,
            }),
          );
          break;
        }
        store.dispatch(
          coreActions.loadPlaylist(
            action.uri,
            { full: true, callbackAction: { name: 'play', shuffle: action.shuffle } },
          ),
        );
        break;
      }

      case 'MOPIDY_PLAY_ALBUM': {
        const album = store.getState().core.items[action.uri];
        const { sortField, sortReverse } = getSortSelector(store.getState(), 'playlist_tracks');
        if (album && album.tracks) {
          store.dispatch(
            mopidyActions.playURIs({
              uris: arrayOf(
                'uri',
                sortItems(
                  album.tracks.filter((t) => t?.is_playable !== false),
                  sortField,
                  sortReverse,
                ),
              ),
              from: {
                uri: album?.uri,
                name: album?.name,
                type: 'album',
              },
              ...action,
            }),
          );
          break;
        }
        store.dispatch(
          coreActions.loadAlbum(
            action.uri,
            { full: true, callbackAction: { name: 'play', shuffle: action.shuffle } },
          ),
        );
        break;
      }

      case 'MOPIDY_ENQUEUE_PLAYLIST': {
        const playlist = store.getState().core.items[action.uri];
        if (playlist && playlist.tracks) {
          store.dispatch(
            mopidyActions.enqueueURIs({
              uris: arrayOf('uri', playlist.tracks),
              ...action,
            }),
          );
          break;
        }
        store.dispatch(
          coreActions.loadPlaylist(
            action.uri,
            false,
            {
              name: 'enqueue',
              ...action,
            },
          ),
        );
        break;
      }

      case 'MOPIDY_ENQUEUE_ALBUM': {
        const album = store.getState().core.items[action.uri];
        if (album && album.tracks) {
          store.dispatch(
            mopidyActions.enqueueURIs({
              uris: arrayOf('uri', album.tracks),
              ...action,
            }),
          );
          break;
        }
        store.dispatch(
          coreActions.loadAlbum(
            action.uri,
            false,
            {
              name: 'enqueue',
              ...action,
            },
          ),
        );
        break;
      }

      case 'MOPIDY_ENQUEUE_URIS': {
        if (!action.uris || action.uris.length <= 0) {
          this.props.uiActions.createNotification({
            content: 'No URIs to enqueue',
            level: 'warning',
          });
          break;
        }

        let remaining = action.uris.length;
        let { at_position } = action;

        // Uris are to go immediately after the currently-playing track (which could be paused)
        if (action.play_next) {
          const {
            current_track,
            queue,
          } = store.getState().core;
          if (current_track) {
            // TODO: Refactor this to findIndex
            for (let i = 0; i < queue.length; i += 1) {
              if (queue[i].tlid === current_track.tlid) {
                at_position = i + 1;
                break;
              }
            }
          }
        }

        store.dispatch(uiActions.startProcess(
          action.type,
          {
            content: i18n('services.mopidy.adding_uris', { count: remaining }),
            remaining,
            total: remaining,
          },
        ));

        store.dispatch(pusherActions.deliverBroadcast(
          'notification',
          {
            notification: {
              content: i18n(
                'services.mopidy.adding_uris_broadcast',
                { username: store.getState().pusher.username, count: remaining },
              ),
              icon: (
                store.getState().core.current_track
                  ? getTrackIcon(store.getState().core.current_track, store.getState().core)
                  : false
              ),
            },
          },
        ));

        // Prepare our URIS into batches. This allows performance gain of several URIs at once
        // (which is a blocking request), but not so many that it locks the Mopidy server.
        // It allows a window of opportunity for other requests to complete before we proceed to
        // the next batch.
        const batchSize = 5;
        const batches = chunk(action.uris, batchSize);

        // This is our process iterator
        const run = () => {
          const processor = store.getState().ui.processes[action.type];
          if (processor && processor.status === 'cancelling') {
            store.dispatch(uiActions.processCancelled(action.type));
            return;
          }

          const uris = batches.shift();

          request(store, 'tracklist.add', { uris, at_position })
            .then(
              (response) => {
                const tlids = response.map((track) => track.tlid);
                store.dispatch(pusherActions.addQueueMetadata(tlids, action.from));

                // Re-run the batch checker in 100ms. This allows a small window for other server
                // requests before our next batch. A little crude but it means the server isn't
                // locked until we're completely done.
                setTimeout(
                  () => {
                    if (!batches.length) {
                      store.dispatch(uiActions.processFinished(action.type));
                    } else {
                      remaining -= uris.length;
                      if (at_position !== null) at_position += uris.length;
                      store.dispatch(
                        uiActions.updateProcess(
                          action.type,
                          {
                            remaining,
                            content: i18n('services.mopidy.adding_uris', { count: remaining }),
                          },
                        ),
                      );
                      run();
                    }
                  },
                  100,
                );
              },
              (error) => {
                store.dispatch(uiActions.processFinished(action.type));
                store.dispatch(coreActions.handleException(
                  `Mopidy: ${error.message || 'Adding tracks failed'}`,
                  error,
                ));
              },
            );
        };

        run(); // Kick off the loop
        break;
      }

      case 'MOPIDY_PLAY_URIS':
        const { from } = action;
        let urisToPlay = Object.assign([], action.uris);

        if (!urisToPlay || !urisToPlay.length) {
          this.props.uiActions.createNotification({ content: 'No URIs to play', level: 'warning' });
          break;
        }

        if (action.shuffle) {
          urisToPlay = shuffle(urisToPlay);
        }

        // Stop the radio
        if (store.getState().core.radio && store.getState().core.radio.enabled) {
          store.dispatch(pusherActions.stopRadio());
        }

        // Clear tracklist (if set)
        if (store.getState().ui.clear_tracklist_on_play) {
          store.dispatch(mopidyActions.clearTracklist());
        }

        // Shuffle/random mode
        if (store.getState().mopidy.random) {
          var first_uri_index = Math.floor(Math.random() * urisToPlay.length);
        } else {
          var first_uri_index = 0;
        }
        var first_uri = urisToPlay[first_uri_index];

        // add our first track
        request(store, 'tracklist.add', { uris: [first_uri], at_position: 0 })
          .then(
            (response) => {
              // play it (only if we got a successful lookup)
              if (response.length > 0) {
                store.dispatch(mopidyActions.changeTrack(response[0].tlid));

                const tlids = [];
                for (let i = 0; i < response.length; i++) {
                  tlids.push(response[i].tlid);
                }
                store.dispatch(pusherActions.addQueueMetadata(tlids, from));
              } else {
                store.dispatch(coreActions.handleException(
                  'Mopidy: Failed to add some tracks',
                  response,
                ));
              }

              // Remove our first_uri as we've already added it
              urisToPlay.splice(first_uri_index, 1);

              // And add the rest of our uris (if any)
              if (urisToPlay.length > 0) {
                // Wait a moment so the server can trigger track_changed etc
                // this means our UI feels snappier as the first track shows up quickly
                setTimeout(
                  () => {
                    store.dispatch(mopidyActions.enqueueURIs({
                      uris: urisToPlay,
                      at_position: 1,
                      from,
                    }));
                  },
                  100,
                );
              }
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                `Mopidy: ${error.message ? error.message : 'Adding tracks failed'}`,
                error,
              ));
            },
          );
        break;

      case 'MOPIDY_REORDER_TRACKLIST':
        request(store, 'tracklist.move', { start: action.range_start, end: action.range_start + action.range_length, to_position: action.insert_before })
          .then(
            () => {
              // TODO: when complete, send event to confirm success/failure
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                `Mopidy: ${error.message ? error.message : 'Reorder failed'}`,
                error,
              ));
            },
          );
        break;

      case 'MOPIDY_CLEAR_TRACKLIST':
        request(store, 'tracklist.clear')
          .then(
            () => {
              store.dispatch(coreActions.clearCurrentTrack());

              store.dispatch(pusherActions.deliverBroadcast(
                'notification',
                {
                  notification: {
                    content: `${store.getState().pusher.username} cleared the playback queue`,
                  },
                },
              ));
            },
          );
        break;

      case 'MOPIDY_SHUFFLE_TRACKLIST':
        request(store, 'tracklist.shuffle', { start: 1 })
          .then(
            () => {
              store.dispatch(pusherActions.deliverBroadcast(
                'notification',
                {
                  notification: {
                    content: `${store.getState().pusher.username} shuffled the playback queue`,
                  },
                },
              ));
            },
          );
        break;

        /**
           * =============================================================== SEARCHING ============
           * ======================================================================================
           * */

      case 'MOPIDY_GET_SEARCH_RESULTS': {
        const {
          uri_schemes = [],
          query = {},
        } = action;
        const types = query.type === 'all'
          ? ['artists', 'albums', 'tracks', 'playlists']
          : [query.type];

        const queue = [];
        uri_schemes.forEach(
          (uri_scheme) => types.forEach(
            (type) => {
              const item = {
                type: query.type,
                term: query.term,
                requestType: type,
                uri_scheme,
                data: {
                  uris: [uri_scheme],
                },
              };
              switch (type) {
                case 'tracks':
                  item.data.query = { any: [query.term] };
                  break;
                case 'artists':
                  item.data.query = { artist: [query.term] };
                  break;
                case 'albums':
                  item.data.query = { album: [query.term] };
                  break;
                case 'playlists':
                  // Searching for playlists is not supported, so we get a simple
                  // list of names and perform a client-side regex match
                  item.method = 'playlists.asList';
                  item.data = {};
                  break;
                default:
                  break;
              }
              queue.push(item);
            },
          ),
        );

        if (queue.length) {
          store.dispatch(uiActions.startProcess(
            action.type,
            {
              content: i18n('services.mopidy.searching'),
              total: queue.length,
              remaining: queue.length,
            },
          ));

          processSearchQueue(store, queue);
        }
        break;
      }

      case 'MOPIDY_GET_URIS': {
        const { uris } = action;
        request(store, 'library.lookup', { uris })
          .then((response) => {
            if (!response) return;

            indexToArray(response).forEach((item) => {
              store.dispatch(coreActions[`load${item.___model__}`](item.uri));
            });
          });
        break;
      }

      case 'MOPIDY_GET_PLAYLIST': {
        const processResponse = (response, fetchTracks = true) => {
          const playlist = formatPlaylist({
            can_edit: true,
            provider: 'mopidy',
            ...response,
            uri: action.uri, // Patch in the requested URI
            type: 'playlist',
          });
          if (response.tracks && fetchTracks) {
            store.dispatch(coreActions.itemLoaded({
              ...playlist,
              tracks: undefined,
              loading: 'tracks',
            }));

            request(store, 'library.lookup', { uris: arrayOf('uri', response.tracks) })
              .then((tracksResponse) => {
                const tracks = response.tracks.map((simpleTrack) => {
                  const fullTracks = tracksResponse[simpleTrack.uri];
                  return {
                    ...simpleTrack,
                    ...(fullTracks.length ? fullTracks[0] : {}),
                  };
                });
                playlist.tracks = injectSortId(formatTracks(tracks));
                store.dispatch(coreActions.itemLoaded(playlist));
              });
          } else {
            store.dispatch(coreActions.itemLoaded(playlist));
          }

          if (!playlist.images) {
            store.dispatch(mopidyActions.getImages([playlist.uri]));
          }
        };

        request(store, 'playlists.lookup', { uri: action.uri })
          .then((playlistResponse) => {
            if (playlistResponse) {
              // Got a playlist from our playlists core, this is typically because it's a local
              // playlist, or one that our backend user owns.
              processResponse(playlistResponse);
            } else {
              console.info('Playlist not in playlists, fetching using library', action.uri);
              // No match, so let's try fetching from foreign provider. This needs to happen when we
              // don't have a HTTP API (eg Spotify) but the playlist is not ours (eg Tidal browse)
              request(store, 'library.lookup', { uris: [action.uri] })
                .then(({ [action.uri]: libraryResponse } = {}) => {
                  if (!libraryResponse) return;
                  processResponse({
                    tracks: libraryResponse,
                    can_edit: false,
                    ...action.options.name ? { name: action.options.name } : {},
                  }, false);
                });
            }
          });
        break;
      }

      case 'MOPIDY_ADD_PLAYLIST_TRACKS':
        request(store, 'playlists.lookup', { uri: action.key })
          .then((response) => {
            if (!response) {
              store.dispatch(coreActions.handleException(
                i18n('errors.uri_not_found', { uri: action.key }),
              ));
              return;
            }
            const tracks = action.tracks_uris.map((uri) => ({ __model__: 'Track', uri }));
            const playlist = { ...response };
            if (playlist.tracks) {
              playlist.tracks = [...playlist.tracks, ...tracks];
            } else {
              playlist.tracks = tracks;
            }

            request(store, 'playlists.save', { playlist })
              .then(() => {
                store.dispatch({
                  type: 'PLAYLIST_TRACKS_ADDED',
                  key: action.key,
                  tracks_uris: action.tracks_uris,
                });
              });
          });
        break;

      case 'MOPIDY_REMOVE_PLAYLIST_TRACKS':
        // reverse order our indexes (otherwise removing from top will affect the keys following)
        function descending(a, b) {
          return b - a;
        }
        var indexes = Object.assign([], action.tracks_indexes);
        indexes.sort(descending);

        request(store, 'playlists.lookup', { uri: action.key })
          .then((response) => {
            const playlist = { ...response };
            for (let i = 0; i < indexes.length; i++) {
              playlist.tracks.splice(indexes[i], 1);
            }
            request(store, 'playlists.save', { playlist })
              .then((response) => {
                store.dispatch({
                  type: 'PLAYLIST_TRACKS_REMOVED',
                  key: action.key,
                  tracks_indexes: action.tracks_indexes,
                });
              });
          });
        break;

      case 'MOPIDY_SAVE_PLAYLIST':
        // Even though we have the full playlist in our index, our "playlists.save" request
        // requires a Mopidy playlist object (with updates)
        request(store, 'playlists.lookup', { uri: action.key })
          .then((response) => {
            const mopidy_playlist = { ...response, name: action.name };

            request(store, 'playlists.save', { playlist: mopidy_playlist })
              .then((response) => {
                if (!response) return;

                // Overwrite our playlist with the response to our save
                // This is essential to get the updated URI from Mopidy
                const playlist = {
                  ...store.getState().core.items[action.key],
                  uri: response.uri,
                  name: response.name,
                };

                // When we rename a playlist, the URI also changes to reflect the name change.
                // We need to update our index, as well as redirect our current page URL.
                if (action.key !== playlist.uri) {
                  // Remove old playlist (by old key/uri) from index
                  // By providing the new key, the old playlist gets replaced with a redirector object
                  store.dispatch(coreActions.removeItem(action.key, playlist.uri));
                  store.dispatch(coreActions.removePinned(action.key));
                  store.dispatch(coreActions.addPinned(playlist));
                }

                store.dispatch(coreActions.itemLoaded(playlist));
                store.dispatch(uiActions.createNotification({ content: 'Playlist saved' }));
              });
          });
        break;

      case 'MOPIDY_REORDER_PLAYLIST_TRACKS':
        request(store, 'playlists.lookup', { uri: action.key })
          .then((response) => {
            let playlist = { ...response };
            const tracks = Object.assign([], playlist.tracks);
            const tracks_to_move = [];

            // calculate destination index: if dragging down, accommodate the offset created by the tracks we're moving
            const { range_start } = action;
            const { range_length } = action;
            let { insert_before } = action;
            if (insert_before > range_start) insert_before -= range_length;

            // collate our tracks to be moved
            for (let i = 0; i < range_length; i += 1) {
              // add to FRONT: we work backwards to avoid screwing up our indexes
              tracks_to_move.unshift(tracks[range_start + i]);
            }

            // remove tracks from their old location
            tracks.splice(range_start, range_length);

            // now plug them back in, in their new location
            for (let i = 0; i < tracks_to_move.length; i += 1) {
              tracks.splice(insert_before, 0, tracks_to_move[i]);
            }

            playlist = { ...playlist, tracks };
            request(store, 'playlists.save', { playlist })
              .then(() => {
                store.dispatch({
                  type: 'PLAYLIST_TRACKS_REORDERED',
                  key: action.key,
                  range_start,
                  range_length,
                  insert_before: action.insert_before, // We've adjusted this, so use original
                });
              });
          });
        break;

      case 'MOPIDY_CREATE_PLAYLIST':
        const data = {
          name: action.playlist.name,
          uri_scheme: action.playlist.scheme,
        };
        request(store, 'playlists.create', data)
          .then((response) => {
            const playlist = formatPlaylist({
              ...response,
              ...action,
              can_edit: true,
              type: 'playlist',
              provider: 'mopidy',
            });
            store.dispatch(uiActions.createNotification({
              content: i18n('actions.created', { name: i18n('playlist.title') }),
            }));
            if (action.playlist.tracks_uris) {
              store.dispatch(coreActions.addTracksToPlaylist(
                playlist.uri,
                action.playlist.tracks_uris,
              ));
            }
            store.dispatch(coreActions.addToLibrary(
              getProvider('playlists', 'm3u:')?.uri,
              playlist,
            ));
          });
        break;

      case 'MOPIDY_DELETE_PLAYLIST':
        request(store, 'playlists.delete', { uri: action.uri })
          .then(() => {
            store.dispatch(uiActions.createNotification({
              content: i18n('actions.deleted', { name: i18n('playlist.title') }),
            }));
            store.dispatch(coreActions.removeFromLibrary('m3u:playlists', action.uri));
          });
        break;

      case 'MOPIDY_GET_ALBUM':
        request(store, 'library.lookup', { uris: [action.uri] })
          .then((_response) => {
            const { uri } = action;
            if (!_response) return;
            let response = _response[uri];
            if (!response || !response.length) return;

            response = sortItems(response, 'track_number');

            const album = {
              ...response[0].album,
              source: 'local',
              artists: formatArtists(response[0].artists),
              tracks: formatTracks(response),
            };

            store.dispatch(coreActions.itemLoaded(album));

            if (!album.images) {
              store.dispatch(mopidyActions.getImages([album.uri]));
            }
          });
        break;

      case 'MOPIDY_GET_ARTIST': {
        request(store, 'library.lookup', { uris: [action.uri] })
          .then((_response) => {
            if (!_response) return;
            const response = _response[action.uri];
            if (!response || !response.length) return;

            const albums = [];
            for (let i = 0; i < response.length; i++) {
              if (response[i].album) {
                const album = {
                  ...formatAlbum(response[i].album),
                  uri: response[i].album.uri,
                };
                if (album) {
                  const existingAlbum = albums.find(
                    (albumToCheck) => album.uri === albumToCheck.uri,
                  );
                  if (!existingAlbum) {
                    albums.push(album);
                  }
                }
              }
            }
            if (albums) {
              store.dispatch(coreActions.itemsLoaded(albums));
            }

            // Get the artist object from the track. This is a bit ugly because it's a simplified
            // (Mopidy) artist object but gives us enough to fetch their name and artwork.
            let raw_artist = null;
            response.forEach((track) => {
              const matchingArtist = track.artists.find((artist) => artist.uri === action.uri);
              if (matchingArtist) {
                raw_artist = matchingArtist;
              }
            });

            const artist = {
              uri: action.uri,
              provider: 'mopidy',
              albums_uris: arrayOf('uri', albums),
              tracks: formatTracks(response),
              ...(raw_artist ? formatArtist(raw_artist) : {}),
            };

            store.dispatch(coreActions.itemLoaded(artist));

            // We may not have a raw_artist if the tracks' artist is not found (ie the requested
            // artist may be the albumartist).
            if (raw_artist) {
              store.dispatch(lastfmActions.getArtist(artist.uri, artist.name, artist.musicbrainz_id));

              // Load supprting information from LastFM and Discogs
              if (store.getState().spotify.enabled) {
                store.dispatch(spotifyActions.getArtistImages(artist));
              } else {
                store.dispatch(discogsActions.getArtistImages(artist.uri, artist));
              }
            }
          });
        break;
      }

      case 'MOPIDY_GET_QUEUE':
        request(store, 'tracklist.getTlTracks')
          .then(
            (response) => {
              store.dispatch({
                type: 'QUEUE_LOADED',
                tracks: response,
              });
            },
          );
        break;

      case 'MOPIDY_GET_QUEUE_HISTORY':
        request(store, 'history.getHistory')
          .then(
            (response) => {
              store.dispatch({
                type: 'MOPIDY_QUEUE_HISTORY',
                tracks: response,
              });
            },
          );
        break;

      case 'MOPIDY_GET_CURRENT_TRACK':
        request(store, 'playback.getCurrentTlTrack')
          .then(
            (response) => {
              if (response && response.track) {
                store.dispatch(mopidyActions.currentTrackLoaded(response));
              }
            },
          );
        break;

      case 'MOPIDY_CURRENT_TRACK_LOADED': {
        store.dispatch(uiActions.setCurrentTrackTransition(false));
        const track = formatTrack(action.tl_track);
        if (track.uri) {
          store.dispatch({
            type: 'CURRENT_TRACK_LOADED',
            track,
            uri: track.uri,
          });
          store.dispatch(coreActions.loadTrack(track.uri, { full: true }));
        }
        break;
      }

      case 'MOPIDY_GET_NEXT_TRACK':
        request(store, 'tracklist.getNextTlid')
          .then(
            (response) => {
              if (response && response >= 0) {
                // Get the full track object from our tracklist
                // We know it will be here, as the tlid refers to an item in this list
                const track = applyFilter('tlid', response, store.getState().core.queue, true);

                if (track && track.uri) {
                  store.dispatch({
                    type: 'NEXT_TRACK_LOADED',
                    uri: track.uri,
                  });

                  store.dispatch(coreActions.loadTrack(track.uri));
                }
              }
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                `Mopidy: ${error.message ? error.message : 'Could not get next track'}`,
                error,
              ));
            },
          );
        break;

      case 'MOPIDY_GET_TRACKS': {
        const { uris, options: { full, lyrics } } = action;
        request(store, 'library.lookup', { uris })
          .then(
            (_response) => {
              if (!_response) return;
              const tracks = compact(indexToArray(_response).map(
                (results) => (results.length ? formatTrack(results[0]) : null),
              ));

              store.dispatch(coreActions.itemsLoaded(tracks));
              store.dispatch(mopidyActions.getImages(arrayOf('uri', tracks)));

              tracks.forEach((track) => {
                if (full) {
                  if (store.getState().lastfm.authorization) {
                    store.dispatch(lastfmActions.getTrack(track.uri));
                  }
                }
                if (lyrics && store.getState().genius.authorization) {
                  store.dispatch(geniusActions.findTrackLyrics(track.uri));
                }
              });
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                `Mopidy: ${error.message ? error.message : 'Could not get track'}`,
                error,
              ));
            },
          );
        break;
      }

      case 'MOPIDY_GET_STREAM_TITLE':
        request(store, 'playback.getStreamTitle')
          .then(
            (stream_title) => {
              if (!stream_title) return;

              store.dispatch(coreActions.streamTitleLoaded(stream_title));
            },
          );
        break;

      case 'VIEW__GET_RANDOM_TRACKS':
        request(store, 'library.browse', { uri: 'local:directory?type=track' })
          .then(
            (_response) => {
              if (!_response || !_response.length) return;

              const uris = sampleSize(arrayOf('uri', _response), action.limit);

              request(store, 'library.lookup', { uris })
                .then(
                  (_response) => {
                    if (!_response) return;

                    const random_tracks = indexToArray(_response).map((results) => results[0]);
                    const view = store.getState().core.view || {};
                    store.dispatch(coreActions.viewDataLoaded({
                      random_tracks: [
                        ...(view.random_tracks ? view.random_tracks : []),
                        ...random_tracks,
                      ],
                      uris: [
                        ...(view.uris ? view.uris : []),
                        ...uris,
                      ],
                    }));
                  },
                );
            },
          );
        break;

      case 'MOPIDY_GET_IMAGES': {
        const { uris } = action;
        if (!uris) break;

        request(store, 'library.getImages', { uris })
          .then((response) => {
            const itemsWithImages = [];
            Object.keys(response).forEach((uri) => {
              const images = response[uri];

              if (images) {
                itemsWithImages.push({
                  uri,
                  images: formatImages(digestMopidyImages(store.getState().mopidy, images)),
                });
              } else {
                store.dispatch(lastfmActions.getImages(uri));
              }
            });

            if (itemsWithImages.length) {
              store.dispatch(coreActions.itemsLoaded(itemsWithImages));
            }
          });

        next(action);
        break;
      }

      case 'MOPIDY_GET_DIRECTORY':
        const uri = action.uri ? decodeURIComponent(action.uri) : null;
        store.dispatch({
          type: 'MOPIDY_DIRECTORY_LOADING',
          uri,
        });

        const processResults = (results) => {
          const tracks = [];
          const subdirectories = [];
          const trackUrisToLoad = [];
          const subdirectoryImagesToLoad = [];
          const playlistsToLoad = [];

          results.forEach((item) => {
            if (item.__model__ === 'Track') {
              tracks.push(formatTrack(item));
            } else if (item.__model__ === 'Ref' && item.type === 'track') {
              tracks.push(formatTrack({ ...item, loading: true }));
              trackUrisToLoad.push(item.uri);
            } else if (item.__model__ === 'Ref' && item.type === 'album') {
              subdirectories.push(formatAlbum({ ...item, loading: true }));
              subdirectoryImagesToLoad.push(item.uri);
            } else if (item.__model__ === 'Ref' && item.type === 'artist') {
              subdirectories.push(formatArtist({ ...item, loading: true }));
              subdirectoryImagesToLoad.push(item.uri);
            } else if (item.__model__ === 'Ref' && item.type === 'playlist') {
              // Tidal moods and genres incorrectly marked as Playlist type
              if (
                item.uri.indexOf('tidal:mood') > -1 ||
                item.uri.indexOf('tidal:genre') > -1
              ) {
                subdirectories.push({
                  ...item,
                  type: 'directory',
                });
              } else {
                subdirectories.push(formatPlaylist({ ...item, loading: true }));
                subdirectoryImagesToLoad.push(item.uri);
                playlistsToLoad.push(item.uri);
              }
            } else {
              subdirectoryImagesToLoad.push(item.uri);
              subdirectories.push(item);
            }
          });

          store.dispatch({
            type: 'MOPIDY_DIRECTORY_LOADED',
            directory: {
              uri,
              tracks,
              subdirectories,
            },
          });

          if (trackUrisToLoad.length) {
            console.info(`Loading ${trackUrisToLoad.length} track URIs`);
            request(store, 'library.lookup', { uris: trackUrisToLoad })
              .then((response) => {
                const fullTrackObjects = Object.values(response).map(
                  (t) => (t.length > 0 ? formatTrack(t[0]) : undefined),
                );

                store.dispatch({
                  type: 'MOPIDY_DIRECTORY_LOADED',
                  directory: {
                    uri,
                    tracks: fullTrackObjects.filter((t) => t instanceof Object),
                  },
                });
              });
          }

          const imagesLoaded = new Promise((resolve) => {
            if (subdirectoryImagesToLoad.length) {
              console.info(`Loading ${subdirectoryImagesToLoad.length} subdirectory URIs`);
              request(store, 'library.getImages', { uris: subdirectoryImagesToLoad })
                .then((response) => {
                  resolve(response);
                });
            } else {
              resolve({});
            }
          });

          const playlistsLoaded = new Promise((resolve) => {
            const playlists = {};
            if (playlistsToLoad.length) {
              console.info(`Loading ${playlistsToLoad.length} playlist URIs`);
              const toLoad = playlistsToLoad.length;
              let loaded = 0;
              playlistsToLoad.map((uri) => {
                request(store, 'playlists.lookup', { uri })
                  .then((playlist) => {
                    playlists[uri] = formatPlaylist({
                      name: playlist.name,
                      uri: playlist.uri,
                      tracks: formatTracks(playlist.tracks),
                    });
                    loaded += 1;
                    if (loaded === toLoad) {
                      resolve(playlists);
                    }
                  })
              });
            } else {
              resolve(playlists);
            }
          });

          imagesLoaded.then((response) => {

            const subdirectoriesWithImages = subdirectories.map((subdir) => {
              let images = response[subdir.uri] || undefined;
              if (images) {
                images = formatImages(digestMopidyImages(store.getState().mopidy, images));
              }
              return {
                ...subdir,
                images,
              };
            });

            playlistsLoaded.then((playlists) => {
              const subdirectoriesWithPlaylists = subdirectoriesWithImages.map((subdir) => {
                const playlist = playlists[subdir.uri] || {};
                return {
                  ...subdir,
                  ...playlist,
                };
              });

              store.dispatch({
                type: 'MOPIDY_DIRECTORY_LOADED',
                directory: {
                  uri,
                  subdirectories: subdirectoriesWithPlaylists,
                },
              });

            });
          });
        };

        const getBrowse = () => request(store, 'library.browse', { uri })
          .then((response) => processResults(response));

        if (uri) {
          request(store, 'library.lookup', { uris: [uri] })
            .then((response) => {
              const {
                [uri]: results = [],
              } = response;

              // Not all endpoints give us tracks/subdirectories to library.lookup
              if (!results.length || uri.startsWith('file:') || uri.startsWith('ytmusic:')) {
                console.info(`No 'library.lookup' results for ${uri}, trying 'library.browse'`);
                getBrowse();
              } else {
                processResults(results);
              }
            });
        } else {
          getBrowse();
        }
        break;

      case 'MOPIDY_DIRECTORY':
        if (store.getState().ui.allow_reporting && action.data) {
          ReactGA.event({ category: 'Directory', action: 'Load', label: action.data.uri });
        }
        next(action);
        break;

      /**
       * My Music libraries
       */
      case 'MOPIDY_GET_LIBRARY_ARTISTS':
        store.dispatch(uiActions.startProcess(action.type, { notification: false }));

        request(store, 'library.browse', { uri: action.uri })
          .then((raw_response) => {
            // No items in our library
            if (!raw_response.length) {
              store.dispatch(coreActions.libraryLoaded({
                uri: 'mopidy:library:artists',
                type: 'artists',
                items_uris: [],
              }));
              store.dispatch(uiActions.processFinished(action.type));
            }

            // Convert local URI to actual artist URI
            // See https://github.com/mopidy/mopidy-local-sqlite/issues/39
            const response = raw_response.map((artist) => ({
              ...formatArtist(artist),
              uri: artist.uri.replace('local:directory?albumartist=', '').replace('local:directory?artist=', ''),
            }));

            store.dispatch(coreActions.itemsLoaded(response));
            store.dispatch(coreActions.libraryLoaded({
              uri: action.uri,
              type: 'artists',
              items_uris: arrayOf('uri', response),
            }));
            store.dispatch(uiActions.processFinished(action.type));
          });
        break;

      case 'MOPIDY_GET_LIBRARY_MOODS': {
        store.dispatch(uiActions.startProcess(action.type, { notification: false }));

        request(store, 'library.browse', { uri: action.uri })
          .then((response) => {
            const moods = response.map((mood) => ({
              ...formatSimpleObject(mood),
              type: 'mood',
            }));

            store.dispatch(
              uiActions.updateProcess(
                action.type,
                {
                  total: moods.length,
                  remaining: moods.length,
                },
              ),
            );

            store.dispatch(coreActions.libraryLoaded({
              uri: action.uri,
              type: 'moods',
              items_uris: arrayOf('uri', moods),
            }));
            store.dispatch(coreActions.itemsLoaded(moods));
            store.dispatch(uiActions.stopLoading(action.uri));
            store.dispatch(uiActions.processFinished(action.type));
          });
        break;
      }

      case 'MOPIDY_GET_LIBRARY_FEATURED_PLAYLISTS': {
        store.dispatch(uiActions.startProcess(action.type, { notification: false }));

        request(store, 'library.browse', { uri: action.uri })
          .then((response) => {
            const moods = response.map((mood) => ({
              ...formatSimpleObject(mood),
              type: 'playlist_group',
            }));

            store.dispatch(
              uiActions.updateProcess(
                action.type,
                {
                  total: moods.length,
                  remaining: moods.length,
                },
              ),
            );

            store.dispatch(coreActions.libraryLoaded({
              uri: action.uri,
              type: 'featured_playlists',
              items_uris: arrayOf('uri', moods),
            }));
            store.dispatch(coreActions.itemsLoaded(moods));
            store.dispatch(uiActions.stopLoading(action.uri));
            store.dispatch(uiActions.processFinished(action.type));
          });
        break;
      }

      case 'MOPIDY_GET_PLAYLIST_GROUP': {
        const processKey = `playlist_group_${action.uri}`;
        const decodedUri = action.uri ? decodeURIComponent(action.uri) : null;
        const playlistGroup = formatPlaylistGroup({
          uri: decodedUri,
          loading: true,
        });
        store.dispatch(uiActions.startProcess(
          processKey,
          { content: i18n('services.mopidy.loading_artwork') },
        ));
        store.dispatch(coreActions.itemLoaded(playlistGroup));
        request(store, 'library.browse', { uri: action.uri })
          .then((browse) => {
            const playlists = browse.map((item) => formatPlaylist({
              ...item,
              images: [], // Images is a playlist dependency, so this prevents triggering full load
            }));
            const playlists_uris = arrayOf('uri', playlists);
            const allUris = [...playlists_uris];
            store.dispatch(coreActions.itemLoaded({
              ...playlistGroup,
              loading: false,
              playlists_uris,
            }));
            store.dispatch(coreActions.itemsLoaded(playlists));
            store.dispatch(
              uiActions.updateProcess(
                processKey,
                {
                  total: allUris.length,
                  remaining: allUris.length,
                },
              ),
            );

            const run = () => {
              if (allUris.length) {
                const uris = allUris.splice(0, 5);
                const processor = store.getState().ui.processes[processKey];
                if (processor && processor.status === 'cancelling') {
                  store.dispatch(uiActions.processCancelled(processKey));
                  return;
                }
                store.dispatch(uiActions.updateProcess(processKey, { remaining: allUris.length }));

                request(store, 'library.getImages', { uris })
                  .then(
                    (response) => {
                      const withImages = uris.map((uri) => {
                        let images = response[uri] || [];
                        if (images) {
                          images = formatImages(digestMopidyImages(store.getState().mopidy, images));
                        }
                        return {
                          uri,
                          images,
                        };
                      });
                      store.dispatch(coreActions.itemsLoaded(withImages));
                      run();
                    },
                  );
              } else {
                store.dispatch(uiActions.processFinished(processKey));
              }
            };

            run();
          });
        break;
      }
      case 'MOPIDY_GET_LIBRARY_PLAYLISTS': {
        store.dispatch(uiActions.startProcess(action.type, { notification: false }));

        // Built-in playlist support works differently to other providers
        if (action.uri === 'm3u:playlists') {
          request(store, 'playlists.asList')
            .then((listResponse) => {
              const libraryPlaylists = [];
              const playlist_uris = arrayOf('uri', listResponse).filter(
                (pUri) => (pUri.indexOf('m3u') > -1),
              );
              store.dispatch(
                uiActions.updateProcess(
                  action.type,
                  {
                    total: playlist_uris.length,
                    remaining: playlist_uris.length,
                  },
                ),
              );

              if (playlist_uris.length) {
                playlist_uris.forEach((uri, index) => {
                  request(store, 'playlists.lookup', { uri })
                    .then((response) => {
                      if (response) {
                        libraryPlaylists.push(
                          formatPlaylist({
                            name: response.name,
                            uri: response.uri,
                            can_edit: uriSource(response.uri) === 'm3u',
                            last_modified: response.last_modified,
                            // By not including actual tracks they will be fetched when needed. We don't
                            // want these simple tracks because they don't contain duration, artist, etc.
                            tracks_total: response.tracks ? response.tracks.length : null,
                          }),
                        );
                      }

                      store.dispatch(
                        uiActions.updateProcess(
                          action.type,
                          {
                            remaining: playlist_uris.length - index - 1,
                          },
                        ),
                      );

                      if (index === playlist_uris.length - 1) {
                        store.dispatch(coreActions.itemsLoaded(libraryPlaylists));
                        store.dispatch(coreActions.libraryLoaded({
                          uri: action.uri,
                          type: 'playlists',
                          items_uris: arrayOf('uri', libraryPlaylists),
                        }));
                        store.dispatch(uiActions.processFinished(action.type));
                      }
                    });
                });
              } else {
                store.dispatch(coreActions.libraryLoaded({
                  uri: action.uri,
                  type: 'playlists',
                  items_uris: [],
                }));
                store.dispatch(uiActions.stopLoading('mopidy:library:playlists'));
                store.dispatch(uiActions.processFinished(action.type));
              }
            });
        } else {
          request(store, 'library.browse', { uri: action.uri })
            .then((browseResponse) => {
              const libraryPlaylists = [];

              store.dispatch(
                uiActions.updateProcess(
                  action.type,
                  {
                    total: browseResponse.length,
                    remaining: browseResponse.length,
                  },
                ),
              );

              if (browseResponse.length) {
                browseResponse.forEach((playlist, index) => {
                  request(store, 'library.lookup', { uris: [playlist.uri] })
                    .then((response) => {
                      if (response) {
                        libraryPlaylists.push(
                          formatPlaylist({
                            name: playlist.name,
                            uri: playlist.uri,
                            can_edit: uriSource(playlist.uri) === 'm3u',
                            last_modified: playlist.last_modified,
                            tracks: formatTracks(response[playlist.uri]),
                          }),
                        );
                      }

                      store.dispatch(
                        uiActions.updateProcess(
                          action.type,
                          {
                            remaining: browseResponse.length - index - 1,
                          },
                        ),
                      );

                      if (index === browseResponse.length - 1) {
                        store.dispatch(coreActions.itemsLoaded(libraryPlaylists));
                        store.dispatch(coreActions.libraryLoaded({
                          uri: action.uri,
                          type: 'playlists',
                          items_uris: arrayOf('uri', libraryPlaylists),
                        }));
                        store.dispatch(uiActions.processFinished(action.type));
                      }
                    });
                });
              } else {
                store.dispatch(coreActions.libraryLoaded({
                  uri: action.uri,
                  type: 'playlists',
                  items_uris: [],
                }));
                store.dispatch(uiActions.stopLoading('mopidy:library:playlists'));
                store.dispatch(uiActions.processFinished(action.type));
              }
            });
        }
        break;
      }

      case 'MOPIDY_GET_LIBRARY_ALBUMS':
        store.dispatch(uiActions.startProcess(action.type, { notification: false }));

        request(store, 'library.browse', { uri: action.uri })
          .then((browseResponse) => {
            const allUris = arrayOf('uri', browseResponse);

            store.dispatch(uiActions.updateProcess(
              action.type,
              {
                remaining: allUris.length,
                total: allUris.length,
              },
            ));

            const run = () => {
              if (allUris.length) {
                const uris = allUris.splice(0, 10);
                const processor = store.getState().ui.processes[action.type];

                if (processor && processor.status === 'cancelling') {
                  store.dispatch(uiActions.processCancelled(action.type));
                  store.dispatch(uiActions.stopLoading(action.uri));
                  return;
                }
                store.dispatch(uiActions.updateProcess(action.type, { remaining: allUris.length }));

                request(store, 'library.lookup', { uris })
                  .then(
                    (lookupResponse) => {
                      const libraryItems = [];
                      indexToArray(lookupResponse).forEach(
                        (tracks) => {
                          if (tracks.length <= 0) {
                            console.info('Ignoring item; no results in lookup');
                            return;
                          }

                          libraryItems.push({
                            artists: tracks[0].artists ? formatArtists(tracks[0].artists) : null,
                            tracks: formatTracks(tracks),
                            last_modified: tracks[0].last_modified,
                            ...formatAlbum(tracks[0].album),
                          });
                        },
                      );

                      if (libraryItems.length) {
                        store.dispatch(coreActions.itemsLoaded(libraryItems));
                      }
                      run();
                    },
                  );
              } else {
                store.dispatch(uiActions.processFinished(action.type));
                store.dispatch(coreActions.libraryLoaded({
                  uri: action.uri,
                  type: 'albums',
                  items_uris: arrayOf('uri', browseResponse),
                }));
              }
            };

            run();
          });
        break;

      case 'MOPIDY_GET_LIBRARY_TRACKS':
        store.dispatch(uiActions.startProcess(action.type, { notification: false }));

        request(store, 'library.browse', { uri: action.uri })
          .then((browseResponse) => {
            const allUris = arrayOf('uri', browseResponse);

            store.dispatch(uiActions.updateProcess(
              action.type,
              {
                remaining: allUris.length,
                total: allUris.length,
              },
            ));

            const run = () => {
              if (allUris.length) {
                const uris = allUris.splice(0, 100);
                const processor = store.getState().ui.processes[action.type];

                if (processor && processor.status === 'cancelling') {
                  store.dispatch(uiActions.processCancelled(action.type));
                  store.dispatch(uiActions.stopLoading('mopidy:library:tracks'));
                  return;
                }
                store.dispatch(uiActions.updateProcess(action.type, { remaining: allUris.length }));

                request(store, 'library.lookup', { uris })
                  .then(
                    (lookupResponse) => {
                      const libraryItems = compact(
                        indexToArray(lookupResponse).map(
                          (results) => (results.length ? formatTrack(results[0]) : null),
                        ),
                      );

                      if (libraryItems.length) {
                        store.dispatch(coreActions.itemsLoaded(libraryItems));
                      }
                      run();
                    },
                  );
              } else {
                store.dispatch(uiActions.processFinished(action.type));
                store.dispatch(coreActions.libraryLoaded({
                  uri: action.uri,
                  type: 'tracks',
                  items_uris: arrayOf('uri', browseResponse),
                }));
              }
            };

            run();
          });
        break;

      default:
        next(action);
        break;
    }
  };
}());

export default MopidyMiddleware;

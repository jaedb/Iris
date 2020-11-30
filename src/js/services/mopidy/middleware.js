
import ReactGA from 'react-ga';
import Mopidy from 'mopidy';
import { sha256 } from 'js-sha256';
import { sampleSize, compact, chunk } from 'lodash';
import { i18n } from '../../locale';
import {
  generateGuid,
  uriSource,
  setFavicon,
  titleCase,
  encodeMopidyUri,
} from '../../util/helpers';
import {
  digestMopidyImages,
  formatImages,
  formatAlbum,
  formatTrack,
  formatTracks,
  formatSimpleObject,
  getTrackIcon,
  formatArtists,
  formatArtist,
  formatPlaylist,
  injectSortId,
} from '../../util/format';
import {
  arrayOf,
  shuffle,
  applyFilter,
  sortItems,
  indexToArray,
} from '../../util/arrays';

const mopidyActions = require('./actions.js');
const coreActions = require('../core/actions.js');
const uiActions = require('../ui/actions.js');
const spotifyActions = require('../spotify/actions.js');
const pusherActions = require('../pusher/actions.js');
const googleActions = require('../google/actions.js');
const lastfmActions = require('../lastfm/actions.js');
const geniusActions = require('../genius/actions.js');
const discogsActions = require('../discogs/actions.js');

const MopidyMiddleware = (function () {
  // container for the actual Mopidy socket
  let socket = null;

  // play position timer
  let progress_interval = null;
  let progress_interval_counter = 0;

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

        // Every 1s update our play position (when playing)
        progress_interval = setInterval(() => {
          if (store.getState().mopidy.play_state === 'playing') {
            // Every 10s get real position from server, provided we're in-focus
            if (progress_interval_counter % 5 === 0 && store.getState().ui.window_focus === true) {
              store.dispatch(mopidyActions.getTimePosition());

              // Otherwise we just assume to add 1000ms every 1000ms of play time
            } else {
              store.dispatch(mopidyActions.timePosition(store.getState().mopidy.time_position + 1000));
            }

            progress_interval_counter += 1;
          }
        }, 1000);

        break;

      case 'state:offline':
        store.dispatch({ type: 'MOPIDY_DISCONNECTED' });
        store.dispatch(mopidyActions.clearCurrentTrack());

        // reset our playback interval timer
        clearInterval(progress_interval);
        progress_interval_counter = 0;
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
  const request = (store, call, value = {}) => {
    return new Promise((resolve, reject) => {
      const loader_key = generateGuid();
      store.dispatch(uiActions.startLoading(loader_key, `mopidy_${call}`));

      const doRequest = () => {
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

        if (controller) {
          const timeout = setTimeout(
            () => {
              store.dispatch(uiActions.stopLoading(loader_key));
              reject(new Error('Request timed out'));
            },
            30000,
          );

          controller(value)
            .then(
              (response) => {
                clearTimeout(timeout);
                store.dispatch(uiActions.stopLoading(loader_key));
                resolve(response);
              },
              (error) => {
                clearTimeout(timeout);
                store.dispatch(uiActions.stopLoading(loader_key));
                reject(error);
              },
            );
        } else {
          // Controller (model.method) doesn't exist, or connection not established
          store.dispatch(uiActions.stopLoading(loader_key));
          console.warn(
            'Mopidy request aborted. Either Mopidy is not connected or the request method is invalid. Check the request and your server settings.',
            { call, value },
          );
        }
      };

      // Give a 5-second leeway for allowing Mopidy to connect, if it isn't already connected
      if (socket) {
        doRequest();
      } else {
        console.info('Mopidy not yet connected, waiting 2 seconds');
        setTimeout(
          () => doRequest(),
          2000,
        );
      }
    });
  };

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
          uri: encodeMopidyUri(playlist.uri),
        }));
      },
      tracks: (response) => {
        const { tracks = [] } = response[0];
        return tracks;
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
      case 'MOPIDY_CONNECT':
        if (socket != null) {
          socket.close();
        }

        store.dispatch({ type: 'MOPIDY_CONNECTING' });
        var state = store.getState();

        socket = new Mopidy({
          webSocketUrl: `ws${state.mopidy.ssl ? 's' : ''}://${state.mopidy.host}:${state.mopidy.port}/mopidy/ws/`,
          callingConvention: 'by-position-or-by-name',
        });

        socket.on((type, data) => handleMessage(socket, store, type, data));
        break;

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
        let servers = store.getState().mopidy.servers;
        servers[action.server.id] = { ...servers[action.server.id], ...action.server };
        store.dispatch(mopidyActions.updateServers(servers));
        break;

      case 'MOPIDY_SET_CURRENT_SERVER':
        store.dispatch(mopidyActions.set({
          current_server: action.server.id,
          host: action.server.host,
          port: action.server.port,
          ssl: action.server.ssl,
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
        break;

      case 'MOPIDY_REMOVE_SERVER':
        let remaining_servers = store.getState().mopidy.servers;
        delete remaining_servers[action.id];
        store.dispatch(mopidyActions.updateServers(remaining_servers));
        break;

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
       **/

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
        const playlist = store.getState().core.items[action.uri];
        if (playlist && playlist.tracks) {
          store.dispatch(
            mopidyActions.playURIs(
              arrayOf('uri', playlist.tracks),
              action.uri,
              action.shuffle,
            ),
          );
          break;
        }
        store.dispatch(
          coreActions.loadItem(
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
            mopidyActions.enqueueURIs(
              arrayOf('uri', playlist.tracks),
              action.uri,
              action.shuffle,
            ),
          );
          break;
        }
        store.dispatch(
          coreActions.loadItem(
            action.uri,
            false,
            {
              name: 'enqueue',
              shuffle: action.shuffle,
              play_next: action.play_next,
              at_position: action.at_position,
              offset: action.offset,
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
        let at_position = action.at_position;

        // Uris are to go immediately after the currently-playing track (which could be paused)
        if (action.play_next) {
          const {
            current_track,
            queue,
          } = store.getState().core;
          if (current_track) {
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
                store.dispatch(pusherActions.addQueueMetadata(tlids, action.from_uri));

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
        const { from_uri } = action;
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
                store.dispatch(pusherActions.addQueueMetadata(tlids, from_uri));
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
                    store.dispatch(mopidyActions.enqueueURIs(urisToPlay, from_uri, null, 1));
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

      case 'MOPIDY_GET_PLAYLIST':
        request(store, 'playlists.lookup', { uri: action.uri })
          .then((response) => {
            if (!response) return;

            const playlist = {
              ...formatPlaylist(response),
              uri: response.uri,
              type: 'playlist',
              provider: 'mopidy',
              can_edit: true,
            };

            if (response.tracks) {
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
            }

          });
        break;

      case 'MOPIDY_ADD_PLAYLIST_TRACKS':
        request(store, 'playlists.lookup', { uri: action.key })
          .then((response) => {
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
        request(store, 'playlists.create', { name: action.name, uri_scheme: action.scheme })
          .then((response) => {
            const playlist = {
              ...formatPlaylist(response),
              ...action,
            };
            store.dispatch(uiActions.createNotification({
              content: i18n('actions.created', { name: i18n('playlist.title') }),
            }));
            store.dispatch(coreActions.addToLibrary('mopidy:library:playlists', playlist));
          });
        break;

      case 'MOPIDY_DELETE_PLAYLIST':
        request(store, 'playlists.delete', { uri: action.uri })
          .then(() => {
            store.dispatch(uiActions.createNotification({
              content: i18n('actions.deleted', { name: i18n('playlist.title') }),
            }));
            store.dispatch(coreActions.removeFromLibrary('mopidy:library:playlists', action.uri));
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
                  ...response[i].album,
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
          store.dispatch(coreActions.loadItem(track.uri, { full: true }));
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

                  store.dispatch(coreActions.loadItem(track.uri));
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
        const { options: { full } } = action;
        request(store, 'library.lookup', { uris: action.uris })
          .then(
            (_response) => {
              if (!_response) return;
              const tracks = compact(indexToArray(_response).map(
                (results) => (results.length ? formatTrack(results[0]) : null),
              ));

              store.dispatch(coreActions.itemsLoaded(tracks));
              store.dispatch(mopidyActions.getImages(arrayOf('uri', tracks)));

              if (full) {
                tracks.forEach((track) => {
                  if (store.getState().lastfm.authorization) {
                    store.dispatch(lastfmActions.getTrack(track.uri));
                  }
                  if (store.getState().genius.authorization) {
                    store.dispatch(geniusActions.findTrackLyrics(track.uri));
                  }
                });
              }
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
                  }
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
              };
            });

            if (itemsWithImages.length) {
              store.dispatch(coreActions.itemsLoaded(itemsWithImages));
            }
          });

        next(action);
        break;
      }


      /**
           * =============================================================== LOCAL ================
           * ======================================================================================
           * */

      case 'MOPIDY_GET_DIRECTORY':
        store.dispatch({
          type: 'MOPIDY_DIRECTORY_FLUSH',
        });
        const uri = action.uri ? decodeURIComponent(action.uri) : null;

        if (uri) {
          request(store, 'library.lookup', { uris: [uri] })
            .then((response) => {
              const {
                [uri]: results = [],
              } = response;

              if (!results.length) return;

              let result = results[0];
              if (result.album) {
                result = {
                  ...result,
                  name: result.album.name,
                };
              }

              store.dispatch({
                type: 'MOPIDY_DIRECTORY_LOADED',
                directory: {
                  ...formatSimpleObject(result),
                },
              });
            });
        }

        request(store, 'library.browse', { uri })
          .then((response) => {
            const tracks_uris = [];
            const tracks = [];
            const subdirectories = [];

            for (const item of response) {
              if (item.type === 'track') {
                tracks_uris.push(item.uri);
              } else {
                subdirectories.push(item);
              }
            }

            if (subdirectories.length > 0) {
              request(store, 'library.getImages', { uris: arrayOf('uri', subdirectories) })
                .then((response) => {

                  const subdirectories_with_images = subdirectories.map((subdir) => {
                    let images = response[subdir.uri] || undefined;
                    if (images) {
                      images = formatImages(digestMopidyImages(store.getState().mopidy, images));
                    }
                    return {
                      ...subdir,
                      images,
                    };
                  });

                  store.dispatch({
                    type: 'MOPIDY_DIRECTORY_LOADED',
                    directory: {
                      subdirectories: subdirectories_with_images,
                    },
                  });
                });
            }

            if (tracks_uris.length > 0) {
              request(store, 'library.lookup', { uris: tracks_uris })
                .then((response) => {
                  if (response.length <= 0) {
                    return;
                  }

                  for (const uri in response) {
                    if (response.hasOwnProperty(uri) && response[uri].length > 0) {
                      tracks.push(formatTrack(response[uri][0]));
                    }
                  }

                  store.dispatch({
                    type: 'MOPIDY_DIRECTORY_LOADED',
                    directory: {
                      tracks,
                      subdirectories,
                    },
                  });
                });
            } else {
              store.dispatch({
                type: 'MOPIDY_DIRECTORY_LOADED',
                directory: {
                  tracks,
                  subdirectories,
                },
              });
            }
          });
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

        request(store, 'library.browse', { uri: store.getState().mopidy.library_artists_uri })
          .then((raw_response) => {
            // No items in our library
            if (!raw_response.length) {
              store.dispatch(coreActions.libraryLoaded({
                uri: 'mopidy:library:artists',
                items_uris: [],
              }));
              store.dispatch(uiActions.processFinished(action.type));
            }

            // Convert local URI to actual artist URI
            // See https://github.com/mopidy/mopidy-local-sqlite/issues/39
            const response = raw_response.map((artist) => ({
              ...artist,
              uri: artist.uri.replace('local:directory?albumartist=', '').replace('local:directory?artist=', '')
            }));

            store.dispatch(coreActions.itemsLoaded(response));
            store.dispatch(coreActions.libraryLoaded({
              uri: 'mopidy:library:artists',
              items_uris: arrayOf('uri', response),
            }));
            store.dispatch(uiActions.processFinished(action.type));
          });
        break;

      case 'MOPIDY_GET_LIBRARY_PLAYLISTS': {
        store.dispatch(uiActions.startProcess(action.type, { notification: false }));

        request(store, 'playlists.asList')
          .then((listResponse) => {

            // Remove any Spotify playlists. These will be handled by our Spotify API
            const playlist_uris = arrayOf('uri', listResponse).filter(
              (playlistUri) => uriSource(playlistUri) !== 'spotify',
            );
            const libraryPlaylists = [];

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
                        uri: 'mopidy:library:playlists',
                        items_uris: arrayOf('uri', libraryPlaylists),
                      }));
                      store.dispatch(uiActions.processFinished(action.type));
                    }
                  });
              });
            } else {
              store.dispatch(coreActions.libraryLoaded({
                uri: 'mopidy:library:playlists',
                items_uris: [],
              }));
              store.dispatch(uiActions.stopLoading('mopidy:library:playlists'));
              store.dispatch(uiActions.processFinished(action.type));
            }
          });
        break;
      }

      case 'MOPIDY_GET_LIBRARY_ALBUMS':
        store.dispatch(uiActions.startProcess(action.type, { notification: false }));

        request(store, 'library.browse', { uri: store.getState().mopidy.library_albums_uri })
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
                  store.dispatch(uiActions.stopLoading('mopidy:library:albums'));
                  return;
                }
                store.dispatch(uiActions.updateProcess(action.type, { remaining: allUris.length }));

                request(store, 'library.lookup', { uris })
                  .then(
                    (lookupResponse) => {
                      const libraryItems = indexToArray(lookupResponse).map((tracks) => ({
                        artists: tracks[0].artists ? formatArtists(tracks[0].artists) : null,
                        tracks: formatTracks(tracks),
                        last_modified: tracks[0].last_modified,
                        ...formatAlbum(tracks[0].album),
                      }));

                      if (libraryItems.length) {
                        store.dispatch(coreActions.itemsLoaded(libraryItems));
                      }
                      run();
                    },
                  );
              } else {
                store.dispatch(uiActions.processFinished(action.type));
                store.dispatch(coreActions.libraryLoaded({
                  uri: 'mopidy:library:albums',
                  items_uris: arrayOf('uri', browseResponse),
                }));
              }
            };

            run();
          });
        break;

      case 'MOPIDY_GET_LIBRARY_TRACKS':
        store.dispatch(uiActions.startProcess(action.type, { notification: false }));

        request(store, 'library.browse', { uri: store.getState().mopidy.library_tracks_uri })
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
                  uri: 'mopidy:library:tracks',
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

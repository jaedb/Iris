
import ReactGA from 'react-ga';
import Mopidy from 'mopidy';
import { sha256 } from 'js-sha256';

import {
  generateGuid,
  uriSource,
  setFavicon,
} from '../../util/helpers';
import {
  digestMopidyImages,
  formatImages,
  formatTrack,
  formatTracks,
  formatSimpleObject,
  getTrackIcon,
} from '../../util/format';
import {
  arrayOf,
  shuffle,
  removeDuplicates,
  applyFilter,
  sortItems,
} from '../../util/arrays';

const mopidyActions = require('./actions.js');
const coreActions = require('../core/actions.js');
const uiActions = require('../ui/actions.js');
const spotifyActions = require('../spotify/actions.js');
const pusherActions = require('../pusher/actions.js');
const googleActions = require('../google/actions.js');
const lastfmActions = require('../lastfm/actions.js');
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

        store.dispatch(mopidyActions.getPlayState());
        store.dispatch(mopidyActions.getVolume());
        store.dispatch(mopidyActions.getMute());
        store.dispatch(mopidyActions.getConsume());
        store.dispatch(mopidyActions.getRandom());
        store.dispatch(mopidyActions.getRepeat());
        store.dispatch(mopidyActions.getQueue());
        store.dispatch(mopidyActions.getCurrentTrack());
        store.dispatch(mopidyActions.getTimePosition());
        store.dispatch(mopidyActions.getUriSchemes());

        // Every 1s update our play position (when playing)
        progress_interval = setInterval(() => {
          if (store.getState().mopidy.play_state == 'playing') {
            // Every 10s get real position from server, provided we're in-focus
            if (progress_interval_counter % 5 == 0 && store.getState().ui.window_focus === true) {
              store.dispatch(mopidyActions.getTimePosition());

              // Otherwise we just assume to add 1000ms every 1000ms of play time
            } else {
              store.dispatch(mopidyActions.timePosition(store.getState().mopidy.time_position + 1000));
            }

            progress_interval_counter++;
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
        store.dispatch(mopidyActions.currentTrackLoaded(data.tl_track));

        // Wait a jiffy before we get the next track
        // We don't want to impede snappyness for this luxury request
        setTimeout(
          () => {
            store.dispatch(mopidyActions.getNextTrack());
          },
          1000,
        );
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

      default:
      // console.log('MopidyService: Unhandled message', type, message );
    }
  };


  /**
     * Call something with Mopidy
     *
     * Sends request to Mopidy server, and updates our local storage on return
     * @param object ws = Mopidy class that wraps a Websocket
     * @param string model = which Mopidy model (playback, tracklist, etc)
     * @param string property = TlTracks, Consume, etc
     * @param string value (optional) = value of the property to pass
     * @return promise
     * */
  const request = (ws, store, call, value = {}) => {
    if (!store.getState().mopidy.connected) {
      return false;
    }

    const callParts = call.split('.');
    const model = callParts[0];
    const method = callParts[1];

    return new Promise((resolve, reject) => {
      if (method in ws[model]) {
        var mopidyObject = ws[model][method];
        var property = method;
      } else {
        var mopidyObject = ws[model];
        var property = model;
      }

      // Detect invalid model.method calls, which result in an empty mopidyObject
      if (!mopidyObject || typeof (mopidyObject) !== 'function') {
        const error = {
          message: 'Call to an invalid object. Check you are calling a valid Mopidy object.',
          call,
          value,
        };

        store.dispatch(coreActions.handleException(
          `Mopidy: ${error.message}`,
          error,
        ));

        reject(error);
      }

      const loader_key = generateGuid();
      store.dispatch(uiActions.startLoading(loader_key, `mopidy_${property}`));

      // Start our 15 second timeout
      const timeout = setTimeout(
        () => {
          store.dispatch(uiActions.stopLoading(loader_key));
          reject({ message: 'Request timed out', call, value });
        },
        30000,
      );

      mopidyObject(value)
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
    });
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
        request(socket, store, action.call, action.value)
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

        // Focus has just been regained
        if (action.window_focus === true) {
          store.dispatch(mopidyActions.getPlayState());
          store.dispatch(mopidyActions.getVolume());
          store.dispatch(mopidyActions.getMute());
          store.dispatch(mopidyActions.getConsume());
          store.dispatch(mopidyActions.getRandom());
          store.dispatch(mopidyActions.getRepeat());
          store.dispatch(mopidyActions.getCurrentTrack());
          store.dispatch(mopidyActions.getTimePosition());
        }
        break;

      case 'MOPIDY_REQUEST':
        request(socket, store, action.method, action.params)
          .then(
            (response) => {
              if (action.response_callback) {
                store.dispatch(action.response_callback.call(this, response));
              }
            },
            (error) => {
              if (action.error_callback) {
                store.dispatch(action.error_callback.call(this, error));
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
        request(socket, store, 'playback.getState')
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
        request(socket, store, 'playback.play')
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
        request(socket, store, 'playback.pause')
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

        request(socket, store, 'playback.previous');
        break;

      case 'MOPIDY_NEXT':

        // Let the UI know we're in transition
        store.dispatch(uiActions.setCurrentTrackTransition('next'));

        request(socket, store, 'playback.next')
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
        request(socket, store, 'playback.stop')
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
        request(socket, store, 'playback.play', { tlid: action.tlid })
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
        request(socket, store, 'tracklist.remove', { criteria: { tlid: action.tlids } })
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
        request(socket, store, 'tracklist.getRepeat')
          .then((response) => {
            store.dispatch({
              type: 'MOPIDY_REPEAT',
              repeat: response,
            });
          });
        break;

      case 'MOPIDY_SET_REPEAT':
        request(socket, store, 'tracklist.setRepeat', [action.repeat]);
        break;

      case 'MOPIDY_GET_RANDOM':
        request(socket, store, 'tracklist.getRandom')
          .then((response) => {
            store.dispatch({
              type: 'MOPIDY_RANDOM',
              random: response,
            });
          });
        break;

      case 'MOPIDY_SET_RANDOM':
        request(socket, store, 'tracklist.setRandom', [action.random]);
        break;

      case 'MOPIDY_GET_CONSUME':
        request(socket, store, 'tracklist.getConsume')
          .then((response) => {
            store.dispatch({
              type: 'MOPIDY_CONSUME',
              consume: response,
            });
          });
        break;

      case 'MOPIDY_SET_CONSUME':
        request(socket, store, 'tracklist.setConsume', [action.consume]);
        break;

      case 'MOPIDY_GET_MUTE':
        request(socket, store, 'mixer.getMute')
          .then((response) => {
            store.dispatch({
              type: 'MOPIDY_MUTE',
              mute: response,
            });
          });
        break;

      case 'MOPIDY_SET_MUTE':
        request(socket, store, 'mixer.setMute', [action.mute])
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
        request(socket, store, 'mixer.getVolume')
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
        request(socket, store, 'mixer.setVolume', { volume: action.volume })
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
        request(socket, store, 'playback.seek', { time_position: action.time_position })
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
        request(socket, store, 'playback.getTimePosition')
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
        request(socket, store, 'getUriSchemes')
          .then(
            (response) => {
              const uri_schemes = response;
              const remove = ['http', 'https', 'mms', 'rtmp', 'rtmps', 'rtsp', 'sc', 'yt'];

              // remove all our ignored types
              for (var i = 0; i < remove.length; i++) {
                const index = uri_schemes.indexOf(remove[i]);
                if (index > -1) uri_schemes.splice(index, 1);
              }

              // append with ':' to make them a mopidy URI
              for (var i = 0; i < uri_schemes.length; i++) {
                uri_schemes[i] = `${uri_schemes[i]}:`;
              }

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


      /**
           * Advanced playback events
           * */

      case 'MOPIDY_PLAY_PLAYLIST':
        var playlist = store.getState().core.playlists[action.uri];

        // We have the playlist loaded already, and we've got at least 1 track to start playing
        if (playlist && playlist.tracks_uris && playlist.tracks_uris.length > 0) {
          // We've got all of the tracks, so just play those; no further action required
          if (playlist.tracks_total == playlist.tracks_uris.length) {
            store.dispatch(mopidyActions.playURIs(playlist.tracks_uris, action.uri, action.shuffle));
            break;
          }

          // Spotify-provided playlists need to be handled by the Spotify service
          if (playlist.provider == 'spotify') {
            store.dispatch(spotifyActions.getAllPlaylistTracks(action.uri, action.shuffle, 'play'));
            break;
          }

          // It's a Spotify playlist that we haven't loaded yet, so Spotify HTTP API needs to go get it
        } else if (uriSource(action.uri) == 'spotify' && store.getState().spotify.enabled) {
          store.dispatch(spotifyActions.getAllPlaylistTracks(action.uri, action.shuffle, 'play'));
          break;
        }

        // Not in index, and Spotify HTTP not enabled, so just play it as-is
        // Fetch the playlist tracks via backend and add each track by URI
        request(socket, store, 'playlists.lookup', { uri: action.uri })
          .then(
            (response) => {
              if (!response || response.tracks === undefined || !response.tracks) {
                store.dispatch(uiActions.createNotification({ content: 'Failed to load playlist tracks', level: 'error' }));
              } else {
                let tracks_uris = arrayOf('uri', response.tracks);
                if (action.shuffle) {
                  tracks_uris = shuffle(tracks_uris);
                }
                store.dispatch(mopidyActions.playURIs(tracks_uris, action.uri));
              }
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                `Mopidy: ${error.message ? error.message : 'Lookup failed'}`,
                error,
              ));
            },
          );
        break;

      case 'MOPIDY_ENQUEUE_PLAYLIST':

        var playlist = store.getState().core.playlists[action.uri];

        // We have the playlist loaded already, and we've got at least 1 track to start playing
        if (playlist && playlist.tracks_uris && playlist.tracks_uris.length > 0) {
          // We've got all of the tracks, so just play those; no further action required
          if (playlist.tracks_total == playlist.tracks_uris.length) {
            let tracks_uris = Object.assign([], playlist.tracks_uris);
            if (action.shuffle) {
              tracks_uris = shuffle(tracks_uris);
            }
            store.dispatch(mopidyActions.enqueueURIs(tracks_uris, action.uri, action.play_next, action.at_position, action.offset));
            break;
          }

          // Spotify-provided playlists need to be handled by the Spotify service
          if (playlist.provider == 'spotify') {
            store.dispatch(spotifyActions.getAllPlaylistTracks(action.uri, action.shuffle, 'enqueue', action.play_next));
            break;
          }

          // It's a Spotify playlist that we haven't loaded yet, so Spotify HTTP API needs to go get it
        } else if (uriSource(action.uri) == 'spotify' && store.getState().spotify.enabled) {
          store.dispatch(spotifyActions.getAllPlaylistTracks(action.uri, action.shuffle, 'enqueue', action.play_next));
          break;
        }

        // Not in index, and Spotify HTTP not enabled, so just play it as-is
        // Fetch the playlist tracks via backend and add each track by URI
        request(socket, store, 'playlists.lookup', { uri: action.uri })
          .then(
            (response) => {
              if (response.tracks === undefined) {
                store.dispatch(uiActions.createNotification({ content: 'Failed to load playlist tracks', level: 'error' }));
              } else {
                let tracks_uris = arrayOf('uri', response.tracks);
                if (action.shuffle) {
                  tracks_uris = shuffle(tracks_uris);
                }
                store.dispatch(mopidyActions.enqueueURIs(tracks_uris, action.uri, action.play_next, action.at_position, action.offset));
              }
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                `Mopidy: ${error.message ? error.message : 'Lookup failed'}`,
                error,
              ));
            },
          );
        break;

      case 'MOPIDY_ENQUEUE_URIS':

        if (!action.uris || action.uris.length <= 0) {
          this.props.uiActions.createNotification({ content: 'No URIs to enqueue', level: 'warning' });
          break;
        }

        store.dispatch(pusherActions.deliverBroadcast(
          'notification',
          {
            notification: {
              content: `${store.getState().pusher.username} is adding ${action.uris.length} URIs to queue`,
              icon: (store.getState().core.current_track ? getTrackIcon(store.getState().core.current_track, store.getState().core) : false),
            },
          },
        ));

        // split into batches
        var uris = Object.assign([], action.uris);
        var batches = [];
        var batch_size = 5;
        while (uris.length > 0) {
          batches.push({
            uris: uris.splice(0, batch_size),
            at_position: action.at_position,
            play_next: action.play_next,
            offset: action.offset + (batch_size * batches.length),
            from_uri: action.from_uri,
          });
        }

        // pass this modified action to the reducer (and other middleware)
        action.batches = batches;
        next(action);

        // start our processor
        store.dispatch(uiActions.startProcess(
          'MOPIDY_ENQUEUE_URIS_PROCESSOR',
          `Adding ${action.uris.length} URI(s)`,
          {
            batches,
            remaining: action.uris.length,
            total: action.uris.length,
          },
        ));
        break;

      case 'MOPIDY_ENQUEUE_URIS_PROCESSOR':

        var last_run = store.getState().ui.processes.MOPIDY_ENQUEUE_URIS_PROCESSOR;

        // Cancelling
        if (last_run && last_run.status == 'cancelling') {
          store.dispatch(uiActions.processCancelled('MOPIDY_ENQUEUE_URIS_PROCESSOR'));
          return;

          // make sure we have some uris in the queue
        } if (action.data.batches && action.data.batches.length > 0) {
          var batches = Object.assign([], action.data.batches);
          var batch = batches[0];
          let total_uris = 0;
          for (var i = 0; i < batches.length; i++) {
            total_uris += batches[i].uris.length;
          }
          batches.shift();
          store.dispatch(uiActions.updateProcess(
            'MOPIDY_ENQUEUE_URIS_PROCESSOR',
            `Adding ${total_uris} URI(s)`,
            {
              remaining: total_uris,
            },
          ));

          // no batches means we're done here
        } else {
          store.dispatch(uiActions.processFinished('MOPIDY_ENQUEUE_URIS_PROCESSOR'));
          break;
        }

        var { current_track } = store.getState().core;
        var { queue } = store.getState().core;
        var current_track_index = -1;

        if (current_track) {
          for (var i = 0; i < queue.length; i++) {
            if (queue[i].tlid == current_track.tlid) {
              current_track_index = i;
              break;
            }
          }
        }

        var params = { uris: batch.uris };

        // Play this batch next
        if (batch.play_next) {
          // Make sure we're playing something first
          if (current_track_index > -1) {
            params.at_position = current_track_index + batch.offset + 1;

            // Default to top of queue if we're not playing
          } else {
            params.at_position = 0 + batch.offset;
          }

          // A specific position has been defined
          // NOTE: This is likely to be wrong as the original action is unaware of batches or other client requests
        } else if (batch.at_position) {
          params.at_position = batch.at_position + batch.offset;
        }

        request(socket, store, 'tracklist.add', params)
          .then(
            (response) => {
              // add metadata to queue
              const tlids = [];
              for (let i = 0; i < response.length; i++) {
                tlids.push(response[i].tlid);
              }
              store.dispatch(pusherActions.addQueueMetadata(tlids, batch.from_uri));

              // Re-run the batch checker in 100ms. This allows a small window for other
              // server requests before our next batch. It's a little crude but it means the server isn't
              // locked until we're completely done.
              setTimeout(
                () => {
                  store.dispatch(uiActions.runProcess(action.type, { batches }));
                },
                100,
              );
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                `Mopidy: ${error.message ? error.message : 'Adding tracks failed'}`,
                error,
              ));
            },
          );

        break;

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
        request(socket, store, 'tracklist.add', { uris: [first_uri], at_position: 0 })
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

        // add our first track
        request(socket, store, 'tracklist.move', { start: action.range_start, end: action.range_start + action.range_length, to_position: action.insert_before })
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
        request(socket, store, 'tracklist.clear')
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
        request(socket, store, 'tracklist.shuffle', { start: 1 })
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


      case 'MOPIDY_GET_SEARCH_RESULTS':

        // Flush out our previous results
        store.dispatch({ type: 'MOPIDY_CLEAR_SEARCH_RESULTS' });

        var uri_schemes_to_ignore = ['spotify:'];
        var uri_schemes = Object.assign([], store.getState().ui.uri_schemes_search_enabled);
        for (var i = 0; i < uri_schemes.length; i++) {
          if (uri_schemes_to_ignore.includes(uri_schemes[i])) {
            uri_schemes.splice(i, 1);
          }
        }
        var uri_schemes_total = uri_schemes.length;
        var uri_scheme = uri_schemes.shift();

        if (uri_schemes_total <= 0) {
          store.dispatch(uiActions.createNotification({ content: 'No sources selected', level: 'warning' }));
        } else {
          store.dispatch(uiActions.startProcess(
            'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
            `Searching ${uri_schemes_total} Mopidy providers`,
            {
              context: action.context,
              query: action.query,
              limit: action.limit,
              total: uri_schemes_total,
              remaining: uri_schemes.length,
              uri_scheme,
              uri_schemes,
            },
          ));
        }

        break;


      case 'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR':
        var last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;

        // Cancelling
        if (last_run && last_run.status == 'cancelling') {
          store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
          return;

          // No more schemes, so we're done!
        } if (!action.data.uri_scheme) {
          store.dispatch(uiActions.processFinished('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
          return;
        }

        // Construct our next batch's task
        var next_uri_schemes = Object.assign([], action.data.uri_schemes);
        var next_uri_scheme = next_uri_schemes.shift();

        // Update UI for this round
        store.dispatch(uiActions.updateProcess(
          'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
          `Searching ${action.data.uri_scheme.replace(':', '')}`,
          {
            remaining: action.data.uri_schemes.length,
          },
        ));

        switch (action.data.context) {
          // Albums
          case 'albums':

            // Quick check to see if we should be cancelling
            var last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;
            if (last_run && last_run.status == 'cancelling') {
              store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
              return;
            }

            store.dispatch(uiActions.updateProcess(
              'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
              `Searching ${action.data.uri_scheme.replace(':', '')}`,
            ));

            var continue_process = () => {
              store.dispatch(uiActions.runProcess(
                'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                {
                  context: action.data.context,
                  query: action.data.query,
                  limit: action.data.limit,
                  uri_scheme: next_uri_scheme,
                  uri_schemes: next_uri_schemes,
                },
              ));
            };

            request(socket, store, 'library.search', { query: { album: [action.data.query] }, uris: [action.data.uri_scheme] })
              .then(
                (response) => {
                  if (response.length > 0) {
                    let albums = [];

                    // Merge our proper album response container
                    if (response[0].albums) {
                      albums = [...response[0].albums, ...albums];
                    }

                    // Pull the Album objects from our track responses
                    if (response[0].tracks) {
                      for (let i = 0; i < response[0].tracks.length; i++) {
                        if (response[0].tracks[i].album !== undefined && response[0].tracks[i].album.uri !== undefined) {
                          albums.push(response[0].tracks[i].album);
                        }
                      }
                    }

                    let albums_uris = arrayOf('uri', albums);
                    albums_uris = removeDuplicates(albums_uris);

                    store.dispatch(coreActions.albumsLoaded(albums));

                    // and plug in their URIs
                    store.dispatch({
                      type: 'MOPIDY_SEARCH_RESULTS_LOADED',
                      query: action.data.query,
                      context: action.data.context,
                      results: albums_uris,
                    });
                  }

                  continue_process();
                },
                (error) => {
                  store.dispatch(coreActions.handleException(
                    `Mopidy: ${error.message ? error.message : 'Search failed'}`,
                    error,
                  ));
                  continue_process();
                },
              );
            break;

          // Artists
          case 'artists':

            // Quick check to see if we should be cancelling
            var last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;
            if (last_run && last_run.status == 'cancelling') {
              store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
              return;
            }

            store.dispatch(uiActions.updateProcess(
              'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
              `Searching ${action.data.uri_scheme.replace(':', '')}`,
            ));

            var continue_process = () => {
              store.dispatch(uiActions.runProcess(
                'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                {
                  context: action.data.context,
                  query: action.data.query,
                  limit: action.data.limit,
                  uri_scheme: next_uri_scheme,
                  uri_schemes: next_uri_schemes,
                },
              ));
            };

            request(socket, store, 'library.search', { query: { artist: [action.data.query] }, uris: [action.data.uri_scheme] })
              .then(
                (response) => {
                  if (response.length > 0) {
                    let artists_uris = [];

                    // Pull actual artist objects
                    if (response[0].artists) {
                      for (var i = 0; i < response[0].artists.length; i++) {
                        artists_uris.push(response[0].artists.uri);
                      }
                    }

                    // Digest track artists into actual artist results
                    if (response[0].tracks) {
                      for (var i = 0; i < response[0].tracks.length; i++) {
                        if (response[0].tracks[i].artists) {
                          for (let j = 0; j < response[0].tracks[i].artists.length; j++) {
                            const artist = response[0].tracks[i].artists[j];
                            if (artist.uri) {
                              artists_uris.push(artist.uri);
                            }
                          }
                        }
                      }
                    }

                    artists_uris = removeDuplicates(artists_uris);

                    // load each artist
                    for (var i = 0; i < artists_uris.length; i++) {
                      store.dispatch(mopidyActions.getArtist(artists_uris[i]));
                    }

                    // and plug in their URIs
                    store.dispatch({
                      type: 'MOPIDY_SEARCH_RESULTS_LOADED',
                      query: action.data.query,
                      context: action.data.context,
                      results: artists_uris,
                    });
                  }

                  continue_process();
                },
                (error) => {
                  store.dispatch(coreActions.handleException(
                    `Mopidy: ${error.message ? error.message : 'Search failed'}`,
                    error,
                  ));
                  continue_process();
                },
              );
            break;

          // Playlists
          case 'playlists':

            // Quick check to see if we should be cancelling
            var last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;
            if (last_run && last_run.status == 'cancelling') {
              store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
              return;
            }

            store.dispatch(uiActions.updateProcess(
              'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
              'Searching playlists',
            ));

            var continue_process = () => {
              store.dispatch(uiActions.processFinished('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
            };

            request(socket, store, 'playlists.asList')
              .then(
                (response) => {
                  if (response.length > 0) {
                    const playlists_uris = [];

                    for (var i = 0; i < response.length; i++) {
                      const playlist = response[i];
                      if (playlist.name.includes(action.data.query) && action.data.uri_schemes.includes(`${uriSource(playlist.uri)}:`)) {
                        playlists_uris.push(playlist.uri);
                      }
                    }

                    // load each playlist
                    for (var i = 0; i < playlists_uris.length; i++) {
                      store.dispatch(mopidyActions.getPlaylist(playlists_uris[i]));
                    }

                    // and plug in their URIs
                    store.dispatch({
                      type: 'MOPIDY_SEARCH_RESULTS_LOADED',
                      query: action.data.query,
                      context: action.data.context,
                      results: playlists_uris,
                    });
                  }
                  continue_process();
                },
                (error) => {
                  store.dispatch(coreActions.handleException(
                    `Mopidy: ${error.message ? error.message : 'Search failed'}`,
                    error,
                  ));
                  continue_process();
                },
              );
            break;

          // Tracks
          case 'tracks':

            // Quick check to see if we should be cancelling
            var last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;
            if (last_run && last_run.status == 'cancelling') {
              store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
              return;
            }

            store.dispatch(uiActions.updateProcess(
              'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
              `Searching ${action.data.uri_scheme.replace(':', '')}`,
            ));

            var continue_process = () => {
              store.dispatch(uiActions.runProcess(
                'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                {
                  context: action.data.context,
                  query: action.data.query,
                  limit: action.data.limit,
                  uri_scheme: next_uri_scheme,
                  uri_schemes: next_uri_schemes,
                },
              ));
            };

            request(socket, store, 'library.search', { query: { any: [action.data.query] }, uris: [action.data.uri_scheme] })
              .then(
                (response) => {
                  if (response.length > 0 && response[0].tracks !== undefined) {
                    const { tracks } = response[0];

                    store.dispatch({
                      type: 'MOPIDY_SEARCH_RESULTS_LOADED',
                      query: action.data.query,
                      context: action.data.context,
                      results: formatTracks(tracks),
                    });
                  }
                  continue_process();
                },
                (error) => {
                  store.dispatch(coreActions.handleException(
                    `Mopidy: ${error.message ? error.message : 'Search failed'}`,
                    error,
                  ));
                  continue_process();
                },
              );

            break;

          // Search for all types
          case 'all':
          default:

            var process_tracks = () => {
              // Quick check to see if we should be cancelling
              const last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;
              if (last_run && last_run.status == 'cancelling') {
                store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
                return;
              }

              store.dispatch(uiActions.updateProcess(
                'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                `Searching ${action.data.uri_scheme.replace(':', '')} tracks`,
                {
                  remaining: (action.data.uri_schemes.length) + 1,
                },
              ));
              request(socket, store, 'library.search', { query: { any: [action.data.query] }, uris: [action.data.uri_scheme] })
                .then(
                  (response) => {
                    if (response.length > 0 && response[0].tracks !== undefined) {
                      const { tracks } = response[0];

                      store.dispatch({
                        type: 'MOPIDY_SEARCH_RESULTS_LOADED',
                        query: action.data.query,
                        context: 'tracks',
                        results: formatTracks(tracks),
                      });
                    }

                    process_albums();
                  },
                  (error) => {
                    store.dispatch(coreActions.handleException(
                      `Mopidy: ${error.message ? error.message : 'Search failed'}`,
                      error,
                    ));
                    process_albums();
                  },
                );
            };

            var process_albums = () => {
              // Quick check to see if we should be cancelling
              const last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;
              if (last_run && last_run.status == 'cancelling') {
                store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
                return;
              }

              store.dispatch(uiActions.updateProcess(
                'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                `Searching ${action.data.uri_scheme.replace(':', '')} albums`,
                {
                  remaining: (action.data.uri_schemes.length) + 0.75,
                },
              ));
              request(socket, store, 'library.search', { query: { album: [action.data.query] }, uris: [action.data.uri_scheme] })
                .then(
                  (response) => {
                    if (response.length > 0) {
                      let albums = [];

                      // Merge actual album responses first
                      if (response[0].albums) {
                        albums = [...response[0].albums, ...albums];
                      }

                      // Then digest tracks albums
                      if (response[0].tracks) {
                        for (let i = 0; i < response[0].tracks.length; i++) {
                          if (response[0].tracks[i].album !== undefined && response[0].tracks[i].album.uri !== undefined) {
                            albums.push(response[0].tracks[i].album);
                          }
                        }
                      }

                      let albums_uris = arrayOf('uri', albums);
                      albums_uris = removeDuplicates(albums_uris);

                      store.dispatch(coreActions.albumsLoaded(albums));

                      // and plug in their URIs
                      store.dispatch({
                        type: 'MOPIDY_SEARCH_RESULTS_LOADED',
                        query: action.data.query,
                        context: 'albums',
                        results: albums_uris,
                      });
                    }

                    process_artists();
                  },
                  (error) => {
                    store.dispatch(coreActions.handleException(
                      `Mopidy: ${error.message ? error.message : 'Search failed'}`,
                      error,
                    ));
                    process_artists();
                  },
                );
            };

            var process_artists = () => {
              // Quick check to see if we should be cancelling
              const last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;
              if (last_run && last_run.status == 'cancelling') {
                store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
                return;
              }

              store.dispatch(uiActions.updateProcess(
                'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                `Searching ${action.data.uri_scheme.replace(':', '')} artists`,
                {
                  remaining: (action.data.uri_schemes.length) + 0.5,
                },
              ));
              request(socket, store, 'library.search', { query: { artist: [action.data.query] }, uris: [action.data.uri_scheme] })
                .then(
                  (response) => {
                    if (response.length > 0) {
                      let artists_uris = [];

                      // Pull our actual artists objects
                      if (response[0].artists) {
                        for (var i = 0; i < response[0].artists.length; i++) {
                          artists_uris.push(response[0].artists.uri);
                        }
                      }

                      // Digest tracks artists
                      if (response[0].tracks) {
                        for (var i = 0; i < response[0].tracks.length; i++) {
                          if (response[0].tracks[i].artists) {
                            for (let j = 0; j < response[0].tracks[i].artists.length; j++) {
                              const artist = response[0].tracks[i].artists[j];
                              if (artist.uri) {
                                artists_uris.push(artist.uri);
                              }
                            }
                          }
                        }
                      }

                      artists_uris = removeDuplicates(artists_uris);

                      // load each artist
                      for (var i = 0; i < artists_uris.length; i++) {
                        store.dispatch(mopidyActions.getArtist(artists_uris[i]));
                      }

                      // and plug in their URIs
                      store.dispatch({
                        type: 'MOPIDY_SEARCH_RESULTS_LOADED',
                        query: action.data.query,
                        context: 'artists',
                        results: artists_uris,
                      });
                    }

                    process_playlists();
                  },
                  (error) => {
                    store.dispatch(coreActions.handleException(
                      `Mopidy: ${error.message ? error.message : 'Search failed'}`,
                      error,
                    ));
                    process_playlists();
                  },
                );
            };
            var process_playlists = () => {
              // Quick check to see if we should be cancelling
              const last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;
              if (last_run && last_run.status == 'cancelling') {
                store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
                return;
              }

              if (action.data.uri_scheme == 'm3u:') {
                store.dispatch(uiActions.updateProcess(
                  'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                  `Searching ${action.data.uri_scheme.replace(':', '')} playlists`,
                  {
                    remaining: (action.data.uri_schemes.length) + 0.25,
                  },
                ));
                request(socket, store, 'playlists.asList')
                  .then(
                    (response) => {
                      if (response.length > 0) {
                        let playlists_uris = [];
                        for (var i = 0; i < response.length; i++) {
                          const playlist = response[i];
                          if (playlist.name.includes(action.data.query) && action.data.uri_schemes.includes(`${uriSource(playlist.uri)}:`)) {
                            playlists_uris.push(playlist.uri);
                          }
                        }

                        playlists_uris = playlists_uris;

                        // load each playlist
                        for (var i = 0; i < playlists_uris.length; i++) {
                          store.dispatch(mopidyActions.getPlaylist(playlists_uris[i]));
                        }

                        // and plug in their URIs
                        store.dispatch({
                          type: 'MOPIDY_SEARCH_RESULTS_LOADED',
                          query: action.data.query,
                          context: 'playlists',
                          results: playlists_uris,
                        });
                      }

                      finished();
                    },
                    (error) => {
                      store.dispatch(coreActions.handleException(
                        `Mopidy: ${error.message ? error.message : 'Search failed'}`,
                        error,
                      ));
                      finished();
                    },
                  );
              } else {
                finished();
              }
            };

            var finished = () => {
              // We're finally done searching for types on this provider
              // On to the next scheme!
              store.dispatch(uiActions.runProcess(
                'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                {
                  context: action.data.context,
                  query: action.data.query,
                  limit: action.data.limit,
                  uri_scheme: next_uri_scheme,
                  uri_schemes: next_uri_schemes,
                  remaining: action.data.uri_schemes.length,
                },
              ));
            };

            // Kick things off with the tracks
            process_tracks();
        }

        break;


      /**
           * =============================================================== PLAYLIST(S) ==========
           * ======================================================================================
           * */

      case 'MOPIDY_GET_LIBRARY_PLAYLISTS':
        request(socket, store, 'playlists.asList')
          .then((response) => {
            // drop in our URI list
            const playlist_uris = arrayOf('uri', response);
            const playlist_uris_filtered = [];

            // Remove any Spotify playlists. These will be handled by our Spotify API
            for (var i = 0; i < playlist_uris.length; i++) {
              if (uriSource(playlist_uris[i]) != 'spotify') {
                playlist_uris_filtered.push(playlist_uris[i]);
              }
            }

            store.dispatch({ type: 'MOPIDY_LIBRARY_PLAYLISTS_LOADED', uris: playlist_uris_filtered });
            store.dispatch({ type: 'MOPIDY_LIBRARY_PLAYLISTS_LOADED_ALL' });

            // get the full playlist objects
            for (var i = 0; i < playlist_uris_filtered.length; i++) {
              request(socket, store, 'playlists.lookup', { uri: playlist_uris_filtered[i] })
                .then((response) => {
                  const source = uriSource(response.uri);
                  const playlist = {

                    type: 'playlist',
                    name: response.name,
                    uri: response.uri,
                    source,
                    provider: 'mopidy',
                    last_modified: response.last_modified,
                    tracks_total: (response.tracks ? response.tracks.length : 0),
                  };

                  store.dispatch(coreActions.playlistLoaded(playlist));
                });
            }
          });
        break;

      case 'MOPIDY_GET_PLAYLIST':
        request(socket, store, 'playlists.lookup', action.data)
          .then((response) => {
            const playlist = {
              ...response,
              uri: response.uri,
              type: 'playlist',
              is_completely_loaded: true,
              provider: 'mopidy',
              tracks: (response.tracks ? response.tracks : []),
              tracks_total: (response.tracks ? response.tracks.length : 0),
            };

            // tracks? get the full track objects
            if (playlist.tracks.length > 0) {
              store.dispatch({
                type: 'MOPIDY_RESOLVE_PLAYLIST_TRACKS',
                tracks: playlist.tracks,
                key: playlist.uri,
              });
            }

            store.dispatch(coreActions.playlistLoaded(playlist));
          });
        break;

      case 'MOPIDY_RESOLVE_PLAYLIST_TRACKS':
        var tracks = Object.assign([], action.tracks);
        var uris = arrayOf('uri', tracks);

        request(socket, store, 'library.lookup', { uris })
          .then((response) => {
            for (const uri in response) {
              if (response.hasOwnProperty(uri)) {
                var track = response[uri][0];
                if (track) {
                  // find the track reference, and drop in the full track data
                  function getByURI(trackReference) {
                    return track.uri == trackReference.uri;
                  }
                  const trackReferences = tracks.filter(getByURI);

                  // there could be multiple instances of this track, so accommodate this
                  for (let j = 0; j < trackReferences.length; j++) {
                    const key = tracks.indexOf(trackReferences[j]);
                    tracks[key] = track;
                  }
                }
              }
            }

            store.dispatch({
              type: 'PLAYLIST_TRACKS',
              tracks,
              key: action.key,
            });
          });
        break;

      case 'MOPIDY_ADD_PLAYLIST_TRACKS':

        request(socket, store, 'playlists.lookup', { uri: action.key })
          .then((response) => {
            const tracks = [];
            for (let i = 0; i < action.tracks_uris.length; i++) {
              tracks.push({
                __model__: 'Track',
                uri: action.tracks_uris[i],
              });
            }

            const playlist = { ...response };
            if (playlist.tracks) {
              playlist.tracks = [...playlist.tracks, ...tracks];
            } else {
              playlist.tracks = tracks;
            }

            request(socket, store, 'playlists.save', { playlist })
              .then((response) => {
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

        request(socket, store, 'playlists.lookup', { uri: action.key })
          .then((response) => {
            const playlist = { ...response };
            for (let i = 0; i < indexes.length; i++) {
              playlist.tracks.splice(indexes[i], 1);
            }
            request(socket, store, 'playlists.save', { playlist })
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
        request(socket, store, 'playlists.lookup', { uri: action.key })
          .then((response) => {
            const mopidy_playlist = { ...response, name: action.name };

            request(socket, store, 'playlists.save', { playlist: mopidy_playlist })
              .then((response) => {
                // Overwrite our playlist with the response to our save
                // This is essential to get the updated URI from Mopidy
                const playlist = {

                  ...store.getState().core.playlists[action.key],
                  uri: response.uri,
                  name: response.name,
                };

                // When we rename a playlist, the URI also changes to reflect the name change.
                // We need to update our index, as well as redirect our current page URL.
                if (action.key !== playlist.uri) {
                  // Remove old playlist (by old key/uri) from index
                  // By providing the new key, the old playlist gets replaced with a redirector object
                  store.dispatch(coreActions.removeFromIndex('playlists', action.key, playlist.uri));
                }

                store.dispatch(coreActions.playlistLoaded(playlist));

                store.dispatch(uiActions.createNotification({ level: 'warning', content: 'Playlist saved' }));
              });
          });
        break;

      case 'MOPIDY_REORDER_PLAYLIST_TRACKS':
        request(socket, store, 'playlists.lookup', { uri: action.key })
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
            for (var i = 0; i < range_length; i++) {
              // add to FRONT: we work backwards to avoid screwing up our indexes
              tracks_to_move.unshift(tracks[range_start + i]);
            }

            // remove tracks from their old location
            tracks.splice(range_start, range_length);

            // now plug them back in, in their new location
            for (var i = 0; i < tracks_to_move.length; i++) {
              tracks.splice(insert_before, 0, tracks_to_move[i]);
            }

            // update playlist
            playlist = { ...playlist, tracks };
            request(socket, store, 'playlists.save', { playlist })
              .then((response) => {
                store.dispatch({
                  type: 'MOPIDY_RESOLVE_PLAYLIST_TRACKS',
                  tracks: playlist.tracks,
                  key: playlist.uri,
                });
              });
          });
        break;

      case 'MOPIDY_CREATE_PLAYLIST':
        request(socket, store, 'playlists.create', { name: action.name, uri_scheme: action.scheme })
          .then((response) => {
            store.dispatch(uiActions.createNotification({ content: 'Created playlist' }));
            store.dispatch(coreActions.playlistLoaded(response));
            store.dispatch({
              type: 'MOPIDY_LIBRARY_PLAYLIST_CREATED',
              key: response.uri,
            });
          });
        break;

      case 'MOPIDY_DELETE_PLAYLIST':
        request(socket, store, 'playlists.delete', { uri: action.uri })
          .then((response) => {
            store.dispatch(uiActions.createNotification({ content: 'Deleted playlist' }));
            store.dispatch(coreActions.removeFromIndex('playlists', action.uri));
            store.dispatch({
              type: 'MOPIDY_LIBRARY_PLAYLIST_DELETED',
              key: action.uri,
            });
          });
        break;


      /**
           * =============================================================== ALBUM(S) =============
           * ======================================================================================
           * */

      case 'MOPIDY_GET_LIBRARY_ALBUMS':
        var last_run = store.getState().ui.processes.MOPIDY_LIBRARY_ALBUMS_PROCESSOR;

        if (!last_run) {
          request(socket, store, 'library.browse', { uri: store.getState().mopidy.library_albums_uri })
            .then((response) => {
              if (response.length <= 0) return;

              const uris = arrayOf('uri', response);
              store.dispatch({
                type: 'MOPIDY_LIBRARY_ALBUMS_LOADED',
                uris,
              });

              // Start our process to load the full album objects
              store.dispatch(uiActions.startProcess(
                'MOPIDY_LIBRARY_ALBUMS_PROCESSOR',
                `Loading ${uris.length} local albums`,
                {
                  uris,
                  total: uris.length,
                  remaining: uris.length,
                },
              ));
            });
        } else if (last_run.status == 'cancelled') {
          store.dispatch(uiActions.resumeProcess('MOPIDY_LIBRARY_ALBUMS_PROCESSOR'));
        } else if (last_run.status == 'finished') {
          // TODO: do we want to force a refresh?
        }

        break;

      case 'MOPIDY_LIBRARY_ALBUMS_PROCESSOR':
        if (store.getState().ui.processes.MOPIDY_LIBRARY_ALBUMS_PROCESSOR !== undefined) {
          const processor = store.getState().ui.processes.MOPIDY_LIBRARY_ALBUMS_PROCESSOR;

          if (processor.status == 'cancelling') {
            store.dispatch(uiActions.processCancelled('MOPIDY_LIBRARY_ALBUMS_PROCESSOR'));
            return false;
          }
        }

        var uris = Object.assign([], action.data.uris);
        var uris_to_load = uris.splice(0, 50);

        if (uris_to_load.length > 0) {
          store.dispatch(uiActions.updateProcess(
            'MOPIDY_LIBRARY_ALBUMS_PROCESSOR',
            `Loading ${uris.length} local albums`,
            {
              uris,
              remaining: uris.length,
            },
          ));
          store.dispatch(mopidyActions.getAlbums(uris_to_load, { name: 'MOPIDY_LIBRARY_ALBUMS_PROCESSOR', data: { uris } }));
        } else {
          store.dispatch(uiActions.processFinished('MOPIDY_LIBRARY_ALBUMS_PROCESSOR'));
        }

        break;

      case 'MOPIDY_GET_ALBUMS':
        request(socket, store, 'library.lookup', { uris: action.uris })
          .then((response) => {
            const albums_loaded = [];
            const artists_loaded = [];
            const tracks_loaded = [];

            for (const uri in response) {
              if (response.hasOwnProperty(uri) && response[uri].length > 0 && response[uri][0].album) {
                const tracks = response[uri];
                const artists_uris = [];
                if (tracks[0].artists) {
                  for (const artist of response[uri][0].artists) {
                    artists_uris.push(artist.uri);
                    artists_loaded.push(artist);
                  }
                }

                const tracks_uris = [];
                for (const track of tracks) {
                  tracks_uris.push(track.uri);
                  tracks_loaded.push(track);
                }

                const album = {
                  source: 'local',
                  artists_uris,
                  tracks_uris,
                  tracks_total: tracks_uris.length,
                  last_modified: tracks[0].last_modified,
                  ...tracks[0].album,
                };

                albums_loaded.push(album);
              }
            }

            store.dispatch(coreActions.albumsLoaded(albums_loaded));
            store.dispatch(coreActions.artistsLoaded(artists_loaded));
            store.dispatch(coreActions.tracksLoaded(tracks_loaded));

            // Re-run any consequential processes in a few ms. This allows a small window for other
            // server requests before our next batch. It's a little crude but it means the server isn't
            // locked until we're completely done.
            if (action.processor) {
              setTimeout(
                () => {
                  store.dispatch(uiActions.runProcess(action.processor.name, action.processor.data));
                },
                10,
              );
            }
          });
        break;

      case 'MOPIDY_GET_ALBUM':
        request(socket, store, 'library.lookup', { uris: [action.uri] })
          .then((_response) => {
            if (!_response) return;
            let response = _response[action.uri];
            if (!response || !response.length) return;

            response = sortItems(response, 'track_number');
            const artists = [];
            if (response[0].artists) {
              for (const artist of response[0].artists) {
                artists.push(artist);
              }
            }

            const album = {
              ...response[0].album,
              source: 'local',
              artists_uris: arrayOf('uri', artists),
              tracks_uris: arrayOf('uri', response),
              tracks_total: response.length,
            };

            store.dispatch(coreActions.albumLoaded(album));
            store.dispatch(coreActions.artistsLoaded(artists));

            // Load images
            if (!response[0].album.images) {
              store.dispatch(mopidyActions.getImages('albums', [album.uri]));
            }

            request(socket, store, 'library.lookup', { uris: album.tracks_uris })
              .then((response) => {
                const tracks_loaded = [];

                for (const uri in response) {
                  if (response.hasOwnProperty(uri)) {
                    tracks_loaded.push(response[uri][0]);
                  }
                }

                store.dispatch(coreActions.tracksLoaded(tracks_loaded));
              });
          });
        break;


      /**
      * =============================================================== ARTIST(S) ============
      * ======================================================================================
      **/
      case 'MOPIDY_GET_LIBRARY_ARTISTS':
        const uri = store.getState().mopidy.library_artists_uri;
        request(socket, store, 'library.browse', { uri })
          .then((response) => {
            if (response.length <= 0) return;

            const uris = [];
            for (let i = 0; i < response.length; i++) {
              // Convert local URI to actual artist URI
              // See https://github.com/mopidy/mopidy-local-sqlite/issues/39
              response[i].uri = response[i].uri.replace('local:directory?albumartist=', '');
              uris.push(response[i].uri);
            }

            store.dispatch(coreActions.artistsLoaded(response));

            store.dispatch({
              type: 'MOPIDY_LIBRARY_ARTISTS_LOADED',
              uris,
            });
          });
        break;

      /**
           * TODO: Fetch and process library artists
           *
           * We can't get specific artist artwork from Mopidy. Perhaps we fetch additional
           * artist metadata via LastFM? Their API limits will make this quite slow.
          case 'MOPIDY_GET_LIBRARY_ARTISTS':

              var last_run = store.getState().ui.processes.MOPIDY_LIBRARY_ARTISTS_PROCESSOR

              if (!last_run){
                  request(socket, store, 'library.browse', { uri: 'local:directory?type=artist' } )
                      .then(response => {
                          if (response.length <= 0) return;

                          var uris = arrayOf('uri',response);

                          store.dispatch({
                              type: 'MOPIDY_LIBRARY_ARTISTS_LOADED',
                              uris: uris
                          });

                          // Start our process to load the full album objects
                          store.dispatch(uiActions.startProcess('MOPIDY_LIBRARY_ARTISTS_PROCESSOR','Loading '+uris.length+' local artists', {uris: uris}));
                      })

              } else if (last_run.status == 'cancelled'){
                  store.dispatch(uiActions.resumeProcess('MOPIDY_LIBRARY_ARTISTS_PROCESSOR'))
              } else if (last_run.status == 'finished'){
                  // TODO: do we want to force a refresh?
              }

              break;

          case 'MOPIDY_LIBRARY_ARTISTS_PROCESSOR':
              if (store.getState().ui.processes['MOPIDY_LIBRARY_ARTISTS_PROCESSOR'] !== undefined){
                  var processor = store.getState().ui.processes['MOPIDY_LIBRARY_ARTISTS_PROCESSOR'];

                  if (processor.status == 'cancelling'){
                      store.dispatch(uiActions.processCancelled('MOPIDY_LIBRARY_ARTISTS_PROCESSOR'));
                      return false;
                  }
              }

              var uris = Object.assign([], action.data.uris)
              var uris_to_load = uris.splice(0,50)

              if (uris_to_load.length > 0){
                  store.dispatch(uiActions.updateProcess('MOPIDY_LIBRARY_ARTISTS_PROCESSOR', 'Loading '+uris.length+' local artists', {uris: uris}));
                  store.dispatch(mopidyActions.getArtists(uris_to_load, {name: 'MOPIDY_LIBRARY_ARTISTS_PROCESSOR', data: {uris: uris}}));
              } else {
                  store.dispatch(uiActions.processFinished('MOPIDY_LIBRARY_ARTISTS_PROCESSOR'));
              }

              break;
           * */

      case 'MOPIDY_GET_ARTIST':
        request(socket, store, 'library.lookup', { uris: [action.uri] })
          .then((_response) => {
            if (!_response) return;
            const response = _response[action.uri];
            if (!response || !response.length) return;

            const albums = [];
            for (let i = 0; i < response.length; i++) {
              if (response[i].album) {
                var album = {
                  ...response[i].album,
                  uri: response[i].album.uri,
                };
                if (album) {
                  function getByURI(albumToCheck) {
                    return album.uri == albumToCheck.uri;
                  }
                  const existingAlbum = albums.find(getByURI);
                  if (!existingAlbum) {
                    albums.push(album);
                  }
                }
              }
            }
            if (albums) {
              store.dispatch(coreActions.albumsLoaded(albums));
            }

            // Start with an empty artist object
            let artist = {
              uri: action.uri,
              provider: 'mopidy',
            };

            // Get the artist object from the track. This is a bit ugly because it's a simplified
            // (Mopidy) artist object but gives us enough to fetch their name and artwork.
            for (const raw_artist of response[0].artists) {
              // We're only interested in the artist we asked for
              if (raw_artist.uri === artist.uri) {
                artist = { ...raw_artist };
              }
            }

            // Add our tracks and albums
            artist.albums_uris = arrayOf('uri', albums),
              artist.tracks = response;

            store.dispatch(coreActions.artistLoaded(artist));

            // Load supprting information from LastFM and Discogs
            const existing_artist = store.getState().core.artists[artist.uri];
            if (existing_artist) {
              if (!existing_artist.images) {
                if (store.getState().spotify.enabled) {
                  store.dispatch(spotifyActions.getArtistImages(artist));
                } else {
                  store.dispatch(discogsActions.getArtistImages(artist.uri, artist));
                }
              }

              // Get biography and other stats from LastFM
              if (!existing_artist.biography) {
                // TODO: Move this condition into lastfmActions
                if (artist.musicbrainz_id) {
                  store.dispatch(lastfmActions.getArtist(artist.uri, false, artist.musicbrainz_id));
                } else {
                  store.dispatch(lastfmActions.getArtist(artist.uri, artist.name));
                }
              }
            }
          });
        break;

      case 'MOPIDY_GET_ARTISTS':
        request(socket, store, 'library.lookup', { uris: action.uris })
          .then((response) => {
            if (response.length <= 0) return;

            const artists = [];

            for (const uri in response) {
              if (response.hasOwnProperty(uri) && response[uri].length > 0 && response[uri][0].artists) {
                const artist = {

                  ...(response ? response[uri][0].artists[0] : {}),
                  provider: 'mopidy',
                };
                artists.push(artist);
              }
            }

            store.dispatch(coreActions.artistsLoaded(artists));

            // Re-run any consequential processes in 100ms. This allows a small window for other
            // server requests before our next batch. It's a little crude but it means the server isn't
            // locked until we're completely done.
            if (action.processor) {
              setTimeout(
                () => {
                  store.dispatch(uiActions.runProcess(action.processor.name, action.processor.data));
                },
                100,
              );
            }
          });
        break;


      /**
           * =============================================================== TRACKS ================
           * ======================================================================================
           * */

      case 'MOPIDY_GET_QUEUE':
        request(socket, store, 'tracklist.getTlTracks')
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
        request(socket, store, 'history.getHistory')
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
        request(socket, store, 'playback.getCurrentTlTrack')
          .then(
            (response) => {
              if (response && response.track) {
                store.dispatch(mopidyActions.currentTrackLoaded(response));
              }
            },
          );
        break;

      case 'MOPIDY_CURRENT_TRACK_LOADED':

        // Let the UI know we're finished transition
        store.dispatch(uiActions.setCurrentTrackTransition(false));

        var track = formatTrack(action.tl_track);
        if (track.uri) {
          // Deliver the data we've got already
          store.dispatch({
            type: 'CURRENT_TRACK_LOADED',
            track,
            uri: track.uri,
          });

          // Now attempt to get supporting images
          if (store.getState().core.tracks[track.uri] === undefined || store.getState().core.tracks[track.uri].images === undefined) {
            // We've got Spotify running, and it's a spotify track - go straight to the source!
            if (store.getState().spotify.enabled && uriSource(track.uri) == 'spotify') {
              store.dispatch(spotifyActions.getTrack(track.uri));

              // Some other source, rely on Mopidy backends to do their work
            } else {
              store.dispatch(mopidyActions.getImages('tracks', [track.uri]));
            }
          }
        }
        break;

      case 'MOPIDY_GET_NEXT_TRACK':
        request(socket, store, 'tracklist.getNextTlid')
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

                  // We don't have the track (including images) already in our index
                  if (store.getState().core.tracks[track.uri] === undefined || store.getState().core.tracks[track.uri].images === undefined) {
                    // We've got Spotify running, and it's a spotify track - go straight to the source!
                    if (store.getState().spotify.enabled && uriSource(track.uri) == 'spotify') {
                      store.dispatch(spotifyActions.getTrack(track.uri));

                      // Some other source, rely on Mopidy backends to do their work
                    } else {
                      store.dispatch(mopidyActions.getImages('tracks', [track.uri]));
                    }
                  }
                }
              }
            },
          );
        break;

      case 'MOPIDY_GET_TRACK':
        request(socket, store, 'library.lookup', { uris: [action.uri] })
          .then(
            (_response) => {
              if (!_response) return;
              const response = _response[action.uri];
              if (!response || !response.length) return;

              const track = { ...response[0] };
              store.dispatch(coreActions.trackLoaded(track));
              store.dispatch(mopidyActions.getImages('tracks', [track.uri]));
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                `Mopidy: ${error.message ? error.message : 'Could not get track'}`,
                error,
              ));
            },
          );
        break;


      /**
           * =============================================================== IMAGES ===============
           * ======================================================================================
           * */

      case 'MOPIDY_GET_IMAGES':
        if (action.uris) {
          request(socket, store, 'library.getImages', { uris: action.uris })
            .then((response) => {
              const records = [];
              for (const uri in response) {
                if (response.hasOwnProperty(uri)) {
                  let images = response[uri];
                  if (images.length) {
                    images = formatImages(digestMopidyImages(store.getState().mopidy, images));
                    records.push({
                      uri,
                      images,
                    });
                  } else {
                    store.dispatch(lastfmActions.getImages(action.context, uri));
                  }
                }
              }

              if (records.length) {
                const action_data = {
                  type: (`${action.context}_LOADED`).toUpperCase(),
                };
                action_data[action.context] = records;
                store.dispatch(action_data);
              }
            });
        }

        next(action);
        break;


      /**
           * =============================================================== LOCAL ================
           * ======================================================================================
           * */

      case 'MOPIDY_GET_DIRECTORY':
        store.dispatch({
          type: 'MOPIDY_DIRECTORY_FLUSH',
        });

        if (action.uri) {
          request(socket, store, 'library.lookup', { uris: [action.uri] })
            .then((response) => {
              if (!response[action.uri] || !response[action.uri].length) return;
              store.dispatch({
                type: 'MOPIDY_DIRECTORY_LOADED',
                directory: formatSimpleObject(response[action.uri][0]),
              });
            });
        }

        request(socket, store, 'library.browse', { uri: action.uri })
          .then((response) => {
            const tracks_uris = [];
            const subdirectories = [];

            for (const item of response) {
              if (item.type === 'track') {
                tracks_uris.push(item.uri);
              } else {
                subdirectories.push(item);
              }
            }

            if (subdirectories.length > 0) {
              request(socket, store, 'library.getImages', { uris: arrayOf('uri', subdirectories) })
                .then((response) => {

                  const subdirectories_with_images = subdirectories.map((subdir) => {
                    let images = response[subdir.uri] || undefined;
                    if (images) {
                      images = formatImages(digestMopidyImages(store.getState().mopidy, images));
                    }
                    return {
                      ...subdir,
                      images: images,
                    }
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
              request(socket, store, 'library.lookup', { uris: tracks_uris })
                .then((response) => {
                  if (response.length <= 0) {
                    return;
                  }

                  const tracks = [];

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

      // This action is irrelevant to us, pass it on to the next middleware
      default:
        return next(action);
    }
  };
}());

export default MopidyMiddleware;

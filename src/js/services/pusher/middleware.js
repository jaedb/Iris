
import ReactGA from 'react-ga';
import { uriType, generateGuid } from '../../util/helpers';
import { trackEvent } from '../../components/Trackable';

const coreActions = require('../core/actions');
const uiActions = require('../ui/actions');
const pusherActions = require('./actions');
const lastfmActions = require('../lastfm/actions');
const geniusActions = require('../genius/actions');
const spotifyActions = require('../spotify/actions');

const PusherMiddleware = (function () {

  // container for the actual websocket
  let socket = null;

  let reconnectTimer = null;

  // requests pending
  const deferredRequests = [];

  // handle all manner of socket messages
  const handleMessage = (ws, store, message) => {
    if (store.getState().ui.log_pusher) {
      console.log('Pusher log (incoming)', message);
    }

    // Pull our ID. JSON-RPC nests the ID under the error object,
    // so make sure we handle that.
    // TODO: Use this as our measure of a successful response vs error
    let id = null;
    if (message.id) {
      id = message.id;
    } else if (message.error && message.error.id) {
      id = message.error.id;
    }

    // Response with request_id
    if (id) {
      // Response matches a pending request
      if (deferredRequests[id] !== undefined) {
        store.dispatch(uiActions.stopLoading(id));

        // Response is an error
        if (message.error !== undefined) {
          deferredRequests[id].reject(message.error);

          // Successful response
        } else {
          deferredRequests[id].resolve(message.result);
        }

        // Hmm, the response doesn't appear to be for us?
      } else {
        store.dispatch(coreActions.handleException(
          'Pusher: Response received with no matching request',
          message,
        ));
      }

    // Broadcast of an error
    } else if (message.error !== undefined) {
      store.dispatch(coreActions.handleException(
        `Pusher: ${message.error.message}`,
        message,
        (message.error.data !== undefined && message.error.data.description !== undefined
          ? message.error.data.description
          : null
        ),
      ));
    // General broadcast received
    } else {
      const params = message.params ? message.params : {};
      switch (message.method) {
        case 'connection_added':
          store.dispatch(pusherActions.connectionAdded(params.connection));
          break;
        case 'connection_changed':
          store.dispatch(pusherActions.connectionChanged(params.connection));
          break;
        case 'connection_removed':
          store.dispatch(pusherActions.connectionRemoved(params.connection));
          break;
        case 'queue_metadata_changed':
          store.dispatch(pusherActions.queueMetadataChanged(params.queue_metadata));
          break;
        case 'spotify_token_changed':
          store.dispatch(spotifyActions.tokenChanged(params.spotify_token));
          break;
        case 'share_configuration_received':
          store.dispatch(uiActions.createNotification({
            type: 'share-configuration-received',
            configuration: params,
            sticky: true,
          }));
          break;
        case 'notification':
          store.dispatch(uiActions.createNotification(params.notification));
          break;
        case 'radio_started':
          store.dispatch(pusherActions.radioStarted(params.radio));
          break;
        case 'radio_changed':
          store.dispatch(pusherActions.radioChanged(params.radio));
          break;
        case 'radio_stopped':
          store.dispatch(pusherActions.radioStopped());
          break;
        case 'commands_changed':
          store.dispatch(pusherActions.commandsUpdated(params.commands));
          break;
        case 'pinned_changed':
          store.dispatch(pusherActions.pinnedUpdated(params.pinned));
          break;
        case 'reload':
          window.location.reload(true);
          break;

        // Local scan
        case 'local_scan_started':
          store.dispatch(uiActions.updateProcess(
            'local_scan', { content: 'Scanning local library' },
          ));
          break;
        case 'local_scan_updated':
          store.dispatch(uiActions.updateProcess(
            'local_scan',
            {
              content: 'Scanning local library',
              data: {},
              description: params.output,
            },
          ));
          break;
        case 'local_scan_finished':
          store.dispatch(uiActions.processFinished(
            'local_scan',
            {
              content: 'Local scan finished',
              description: params.output,
              sticky: true,
            },
          ));
          break;
        case 'local_scan_error':
          store.dispatch(uiActions.processFinished(
            'local_scan',
            {
              level: 'error', content: 'Local scan failed', description: params.error, sticky: true,
            },
          ));
          break;

        // Upgrade
        case 'upgrade_started':
          store.dispatch(uiActions.updateProcess('upgrade', { content: 'Upgrading' }));
          break;
        case 'upgrade_updated':
          store.dispatch(uiActions.updateProcess(
            'upgrade', { content: 'Upgrading', data: {}, description: params.output },
          ));
          break;
        case 'upgrade_finished':
          store.dispatch(uiActions.updateProcess(
            'upgrade', { content: 'Restarting to complete upgrade' },
          ));
          break;
        case 'upgrade_error':
          store.dispatch(uiActions.processFinished(
            'upgrade',
            {
              level: 'error', content: 'Upgrade failed', description: params.error, sticky: true,
            },
          ));
          break;

        // Restart
        case 'restart_started':
          store.dispatch(uiActions.removeProcess('upgrade', { content: 'Restarting' }));
          break;
        case 'restart_updated':
          store.dispatch(uiActions.updateProcess(
            'upgrade', { content: 'Restarting', data: {}, description: params.output },
          ));
          break;
        case 'restart_error':
          store.dispatch(uiActions.processFinished(
            'upgrade',
            {
              level: 'error', content: 'Restart failed', description: params.error, sticky: true,
            },
          ));
          break;

        // Test
        case 'test_started':
          store.dispatch(uiActions.updateProcess(
            'test', { content: 'Running test', data: {}, description: params.output },
          ));
          break;
        case 'test_updated':
          store.dispatch(uiActions.updateProcess('test', { content: 'Running test' }));
          break;
        case 'test_finished':
          store.dispatch(uiActions.processFinished(
            'test',
            {
              content: 'Test finished',
              description: params.output,
              sticky: true,
            },
          ));
          break;
        case 'test_error':
          store.dispatch(uiActions.removeProcess(
            'test',
            {
              level: 'error', content: params.message, description: params.error,
            },
          ));
          break;
        default:
          break;
      }
    }
  };

  const request = (store, method, params = null) => new Promise((resolve, reject) => {
    const id = generateGuid();
    const message = {
      jsonrpc: '2.0',
      id,
      method,
    };
    if (params) {
      message.params = params;
    }

    if (store.getState().ui.log_pusher) {
      console.log('Pusher log (outgoing)', message);
    }

    socket.send(JSON.stringify(message));

    store.dispatch(uiActions.startLoading(id, `pusher_${method}`));

    // Start our 30 second timeout
    setTimeout(
      () => {
        store.dispatch(uiActions.stopLoading(id));
        reject({
          id,
          code: 32300,
          message: 'Request timed out',
        });
      },
      30000,
    );

    // add query to our deferred responses
    deferredRequests[id] = {
      resolve,
      reject,
    };
  });

  return (store) => (next) => (action) => {
    const { pusher } = store.getState();

    switch (action.type) {
      case 'PUSHER_CONNECT':
        if (socket != null) {
          socket.close();
        }

        clearTimeout(reconnectTimer);
        store.dispatch({ type: 'PUSHER_CONNECTING' });

        socket = new WebSocket(
          `ws${window.location.protocol === 'https:' ? 's' : ''}://${store.getState().mopidy.host}:${store.getState().mopidy.port}/iris/ws/`,
        );

        socket.onopen = () => {
          store.dispatch({
            type: 'PUSHER_CONNECTED',
          });
        };

        socket.onclose = (e) => {
          store.dispatch({
            type: 'PUSHER_DISCONNECTED',
          });

          // attempt to reconnect every 5 seconds
          reconnectTimer = setTimeout(() => {
            store.dispatch(pusherActions.connect());
          }, 5000);
        };

        socket.onerror = (e) => {
          if (socket.readyState === 1) {
            store.dispatch(coreActions.handleException(
              'Pusher websocket error',
              e,
              e.type,
            ));
          }
        };

        socket.onmessage = (message) => {
          handleMessage(socket, store, JSON.parse(message.data));
        };

        break;

      case 'PUSHER_CONNECTED':
        if (store.getState().ui.allow_reporting) {
          ReactGA.event({ category: 'Pusher', action: 'Connected', label: action.username });
        }

        clearTimeout(reconnectTimer);
        store.dispatch(pusherActions.updateConnection());
        store.dispatch(pusherActions.getConfig());
        store.dispatch(pusherActions.getRadio());
        store.dispatch(pusherActions.getCommands());
        store.dispatch(pusherActions.getPinned());
        store.dispatch(pusherActions.getQueueMetadata());

        // Give things a few moments to setup before we check for version.
        // This is because the server makes a GitHub request, which creates a [very] small delay
        // in subsequent requests.
        setTimeout(
          () => {
            store.dispatch(pusherActions.getVersion());
          },
          500,
        );
        next(action);
        break;

      case 'PUSHER_REQUEST':
        request(store, action.method, action.params)
          .then(
            (response) => {
              if (action.response_callback) {
                action.response_callback.call(this, response);
              }
            },
            (error) => {
              if (action.error_callback) {
                action.error_callback.call(this, error);
              } else {
                store.dispatch(coreActions.handleException(
                  'Pusher request failed',
                  error,
                  action.method,
                  action,
                ));
              }
            },
          );
        break;

      case 'PUSHER_DELIVER_MESSAGE':
        request(store, 'send_message', action.data)
          .then(
            (response) => {
              store.dispatch(uiActions.createNotification({ content: 'Message delivered' }));
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Could not deliver message',
                error,
              ));
            },
          );
        break;

      case 'PUSHER_DELIVER_BROADCAST':
        request(store, 'broadcast', action.data);
        break;

      case 'PUSHER_SET_USERNAME':
        store.dispatch(pusherActions.updateConnection({ username: action.username }));
        next(action);
        break;

      case 'PUSHER_UPDATE_CONNECTION':

        // Our action can provide new values during a state update (eg the field was just changed)
        // but by default we refer to our existing state
        const connection = {
          username: store.getState().pusher.username,
          client_id: store.getState().pusher.client_id,
          ...(action.connection ? action.connection : {}),
        };

        request(store, 'update_connection', connection)
          .then(
            (response) => {
              response.type = 'PUSHER_CONNECTION_UPDATED';
              store.dispatch(response);
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Could not update connection',
                error,
              ));
            },
          );
        next(action);
        break;

      case 'PUSHER_GET_QUEUE_METADATA':
        request(store, 'get_queue_metadata')
          .then(
            (response) => {
              response.type = 'PUSHER_QUEUE_METADATA';
              store.dispatch(response);
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Could not load queue metadata',
                error,
              ));
            },
          );
        break;

      case 'PUSHER_ADD_QUEUE_METADATA':
        request(store, 'add_queue_metadata', {
          tlids: action.tlids,
          added_from: action.from_uri,
          added_by: pusher.username,
        });
        break;

      case 'PUSHER_GET_VERSION':
        request(store, 'get_version')
          .then(
            (response) => {
              store.dispatch({
                type: 'PUSHER_VERSION',
                version: response.version,
              });
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Could not load version',
                error,
              ));
            },
          );
        break;

      case 'PUSHER_GET_CONFIG':
        request(store, 'get_config')
          .then(
            (response) => {
              store.dispatch({
                type: 'PUSHER_CONFIG',
                config: response.config,
              });
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Could not load config',
                error,
              ));
            },
          );
        break;

      case 'PUSHER_GET_CONNECTIONS':
        request(store, 'get_connections')
          .then(
            (response) => {
              store.dispatch({
                type: 'PUSHER_CONNECTIONS',
                connections: response.connections,
              });
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Could not load connections',
                error,
              ));
            },
          );
        next(action);
        break;

      /**
       * Pinned uris
       **/

      case 'PUSHER_GET_PINNED':
        request(store, 'get_pinned')
          .then(
            (response) => {
              store.dispatch(pusherActions.pinnedUpdated(response.pinned));
            },
            (error) => {
              // We're not too worried about capturing errors here
              // It's also likely to fail where UI has been updated but
              // server hasn't been restarted yet.
              // TODO: Wrap Pusher in 5second wait, like Mopidy
            },
          );
        next(action);
        break;

      case 'PUSHER_ADD_PINNED':
        var pinned = [...pusher.pinned, action.item];
        store.dispatch(pusherActions.setPinned(pinned));
        break;

      case 'PUSHER_SET_PINNED':
        request(store, 'set_pinned', { pinned: action.pinned })
          .then(
            (response) => {
              // No action required, the change will be broadcast
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Could not pin URI',
                error,
              ));
            },
          );

        next(action);
        break;

      case 'PUSHER_REMOVE_PINNED':
        var pinned = pusher.pinned.filter((item) => item.uri !== action.uri);

        request(store, 'set_pinned', { pinned })
          .then(
            (response) => {
              // No action required, the change will be broadcast
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Could not unpin URI',
                error,
              ));
            },
          );

        next(action);
        break;

      /**
       * Commands
       * */
      case 'PUSHER_GET_COMMANDS': {
        request(store, 'get_commands')
          .then(
            (response) => {
              store.dispatch(pusherActions.commandsUpdated(response.commands));
            },
            () => {
              // We're not too worried about capturing errors here
              // It's also likely to fail where UI has been updated but
              // server hasn't been restarted yet.
              // TODO: Wrap Pusher in 5second wait, like Mopidy
            },
          );
        next(action);
        break;
      }

      case 'PUSHER_SET_COMMAND': {
        const commands = {
          ...pusher.commands,
          [action.command.id]: {
            ...(pusher.commands[action.command.id] || {}),
            ...action.command,
          },
        };

        store.dispatch(pusherActions.setCommands(commands));
        break;
      }

      case 'PUSHER_SET_COMMANDS': {
        request(store, 'set_commands', { commands: action.commands })
          .then(
            () => {
              // No action required, the change will be broadcast
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Could not set commands',
                error,
              ));
            },
          );

        next(action);
        break;
      }

      case 'PUSHER_REMOVE_COMMAND': {
        const commands_index = { ...pusher.commands };
        delete commands_index[action.id];

        request(store, 'set_commands', { commands: commands_index })
          .then(
            () => {
              // No action required, the change will be broadcast
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Could not remove command',
                error,
              ));
            },
          );

        next(action);
        break;
      }

      case 'PUSHER_RUN_COMMAND': {
        const notification_key = `command_${action.id}`;

        if (action.notify) {
          store.dispatch(uiActions.startProcess(notification_key, 'Running command'));
        }

        request(store, 'run_command', { id: action.id })
          .then(() => {
            store.dispatch(uiActions.removeProcess(notification_key));
            if (action.notify) {
              store.dispatch(uiActions.createNotification({
                key: notification_key,
                content: 'Command sent',
              }));
            }
          },
          (error) => {
            store.dispatch(uiActions.removeProcess(notification_key));
            store.dispatch(coreActions.handleException(
              'Could not run command',
              error,
            ));
          });
        break;
      }


      /**
           * Radio
           * */

      case 'PUSHER_GET_RADIO':
        request(store, 'get_radio')
          .then(
            (response) => {
              store.dispatch({
                type: 'PUSHER_RADIO_LOADED',
                radio: response.radio,
              });

              if (response.radio.enabled) {
                store.dispatch(spotifyActions.resolveRadioSeeds(response.radio));
              }
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Could not load radio',
                error,
              ));
            },
          );
        break;

      case 'PUSHER_START_RADIO':
      case 'PUSHER_UPDATE_RADIO': {
        trackEvent({ category: 'Pusher', action: 'Start radio', label: action.uris.join() });

        store.dispatch(
          uiActions.startProcess(
            action.type,
            {
              content: `${action.type === 'PUSHER_UPDATE_RADIO' ? 'Updating' : 'Starting'} radio`,
            },
          ),
        );

        const data = {
          reset: (action.type === 'PUSHER_START_RADIO'),
          seed_artists: [],
          seed_genres: [],
          seed_tracks: [],
        };

        for (let i = 0; i < action.uris.length; i += 1) {
          switch (uriType(action.uris[i])) {
            case 'artist':
              data.seed_artists.push(action.uris[i]);
              break;
            case 'track':
              data.seed_tracks.push(action.uris[i]);
              break;
            case 'genre':
              data.seed_genres.push(action.uris[i]);
              break;
            default:
              break;
          }
        }

        if (action.type === 'PUSHER_START_RADIO') {
          store.dispatch(pusherActions.deliverBroadcast(
            'notification',
            {
              notification: {
                content: `${pusher.username} is starting radio mode`,
              },
            },
          ));
        }

        request(store, 'change_radio', data)
          .then(
            (response) => {
              store.dispatch(uiActions.processFinished(action.type));
              if (response.status === 0) {
                store.dispatch(uiActions.createNotification({ content: response.message, level: 'error' }));
              }
              store.dispatch(pusherActions.radioChanged(response.radio));
            },
            (error) => {
              store.dispatch(uiActions.processFinished(action.type));
              store.dispatch(coreActions.handleException(
                'Could not change radio',
                error,
              ));
            },
          );
        break;
      }

      case 'PUSHER_STOP_RADIO': {
        store.dispatch(uiActions.createNotification({ content: 'Stopping radio' }));
        trackEvent({ category: 'Pusher', action: 'Stop radio' });

        store.dispatch(pusherActions.deliverBroadcast(
          'notification',
          {
            notification: {
              content: `${pusher.username} stopped radio mode`,
            },
          },
        ));

        const data = {
          seed_artists: [],
          seed_genres: [],
          seed_tracks: [],
        };

        request(store, 'stop_radio', data)
          .then(
            (response) => {
              store.dispatch(pusherActions.radioStopped());
            }, (error) => {
              store.dispatch(coreActions.handleException(
                'Could not stop radio',
                error,
              ));
            },
          );
        break;
      }

      case 'PUSHER_BROWSER_NOTIFICATION':
        store.dispatch(uiActions.createBrowserNotification(action));
        break;

      case 'PUSHER_NOTIFICATION':
        store.dispatch(uiActions.createNotification({
          ...action, type: action.notification_type,
        }));
        break;

      case 'PUSHER_RELOAD':
        // Hard reload. This doesn't strictly clear the cache, but our compiler's
        // cache buster should handle that
        window.location.reload(true);
        break;

      case 'PUSHER_RESTART':
        request(store, 'restart');
        next(action);
        break;

      case 'PUSHER_UPGRADE':
        if (store.getState().ui.allow_reporting) {
          ReactGA.event({ category: 'Pusher', action: 'Upgrade', label: '' });
        }
        request(store, 'upgrade');
        break;

      case 'PUSHER_LOCAL_SCAN':
        if (store.getState().ui.allow_reporting) {
          ReactGA.event({ category: 'Pusher', action: 'Local scan', label: '' });
        }
        request(store, 'local_scan');
        break;

      case 'PUSHER_TEST':
        request(store, 'test');
        break;

      case 'PUSHER_VERSION':
        if (store.getState().ui.allow_reporting) {
          ReactGA.event({ category: 'Pusher', action: 'Version', label: action.version.current });
        }
        next(action);
        break;

      case 'PUSHER_CONFIG': {
        // Set default country/locale (unless we've already been configured)
        const { spotify } = store.getState();
        const spotify_updates = {};
        let spotify_updated = false;

        if (!spotify.country && action.config.country) {
          spotify_updates.country = action.config.country;
          spotify_updated = true;
        }

        if (!spotify.locale && action.config.locale) {
          spotify_updates.locale = action.config.locale;
          spotify_updated = true;
        }

        if (action.config.spotify_authorization_url) {
          spotify_updates.authorization_url = action.config.spotify_authorization_url;
          spotify_updated = true;
        }

        if (spotify_updated) {
          store.dispatch(spotifyActions.set(spotify_updates));
        }

        store.dispatch(lastfmActions.set({
          authorization_url: (action.config.lastfm_authorization_url ? action.config.lastfm_authorization_url : null),
        }));

        store.dispatch(geniusActions.set({
          authorization_url: (action.config.genius_authorization_url ? action.config.genius_authorization_url : null),
        }));

        next(action);
        break;
      }

      case 'PUSHER_DEBUG':
        request(store, action.message.method, action.message.data)
          .then(
            (response) => {
              store.dispatch({ type: 'DEBUG', response });
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Could not debug',
                error,
                error.message,
              ));
            },
          );
        break;

      case 'PUSHER_ERROR':
        store.dispatch(uiActions.createNotification(action.message, 'bad'));
        trackEvent({ category: 'Pusher', action: 'Error', label: action.message });
        break;

      default:
        return next(action);
    }
  };
}());

export default PusherMiddleware;

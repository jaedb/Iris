import ReactGA from 'react-ga';
import { sha256 } from 'js-sha256';
import {
  generateGuid,
} from '../../util/helpers';
import {
  arrayOf,
} from '../../util/arrays';
import {
  formatGroup,
  formatClient,
} from '../../util/format';

const coreActions = require('../core/actions');
const uiActions = require('../ui/actions');
const pusherActions = require('../pusher/actions');
const snapcastActions = require('./actions');

const SnapcastMiddleware = (function () {
  let socket = null;
  let reconnectTimer = null;

  // requests pending
  const deferredRequests = [];

  // handle all manner of socket messages
  const handleMessage = (ws, store, message) => {
    if (store.getState().ui.log_snapcast) {
      console.log('Snapcast log (incoming)', message);
    }

    // Some messages are arrays of messages
    if (Array.isArray(message)) {
      message.map((messageItem) => handleMessage(ws, store, messageItem));
      return;
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
          'Snapcast: Response received with no matching request',
          message,
        ));
      }

      // General broadcast received
    } else {
      switch (message.method) {
        case 'Client.OnConnect':
          store.dispatch(snapcastActions.clientLoaded(message.params.client));
          break;

        case 'Client.OnDisconnect':
          store.dispatch(snapcastActions.clientLoaded(message.params.client));
          break;

        case 'Client.OnVolumeChanged':
          store.dispatch(snapcastActions.clientLoaded(message.params));
          break;

        case 'Client.OnLatencyChanged':
          store.dispatch(snapcastActions.clientLoaded(message.params));
          break;

        case 'Client.OnNameChanged':
          store.dispatch(snapcastActions.clientLoaded(message.params));
          break;

        case 'Group.OnMute':
          store.dispatch(snapcastActions.groupLoaded(message.params));
          break;

        case 'Group.OnNameChanged':
          store.dispatch(snapcastActions.groupLoaded(message.params));
          break;

        case 'Server.OnUpdate':
          store.dispatch(snapcastActions.serverLoaded(message.params.server.server));
          store.dispatch(snapcastActions.groupsLoaded(message.params.server.groups, true));
          store.dispatch(snapcastActions.streamsLoaded(message.params.server.streams, true));
          break;

        default:
          break;
      }
    }
  };

  const request = (store, method, params = null) => new Promise((resolve, reject) => {
    const id = generateGuid(8);
    const message = {
      jsonrpc: '2.0',
      id,
      method,
    };
    if (params) {
      message.params = params;
    }

    if (store.getState().ui.log_snapcast) {
      console.log('Snapcast log (outgoing)', message);
    }

    socket.send(JSON.stringify(message));

    store.dispatch(uiActions.startLoading(id, `snapcast_${method}`));

    // Start our 30 second timeout
    const timeout = setTimeout(
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
    const { snapcast } = store.getState();

    switch (action.type) {
      case 'SNAPCAST_CONNECT':
        if (socket) socket.close();
        clearTimeout(reconnectTimer);

        store.dispatch({ type: 'SNAPCAST_CONNECTING' });

        socket = new WebSocket(
          `ws${window.location.protocol === 'https:' ? 's' : ''}://${store.getState().snapcast.host}:${store.getState().snapcast.port}/jsonrpc`,
        );

        socket.onopen = () => {
          store.dispatch({
            type: 'SNAPCAST_CONNECTED',
          });
        };

        socket.onclose = (e) => {
          store.dispatch({
            type: 'SNAPCAST_DISCONNECTED',
          });

          // attempt to reconnect every 5 seconds
          if (store.getState().snapcast.enabled) {
            clearTimeout(reconnectTimer);
            reconnectTimer = setTimeout(() => {
              store.dispatch(snapcastActions.connect());
            }, 5000);
          }
        };

        socket.onerror = (e) => {
          if (socket.readyState == 1) {
            store.dispatch(coreActions.handleException(
              'Snapcast websocket error',
              e,
              e.type,
            ));
          }
        };

        socket.onmessage = (message) => {
          handleMessage(socket, store, JSON.parse(message.data));
        };
        break;

      case 'SNAPCAST_CONNECTED':
        if (store.getState().ui.allow_reporting) {
          ReactGA.event({
            category: 'Snapcast',
            action: 'Connected',
            label: sha256(window.location.hostname),
          });
        }
        store.dispatch(snapcastActions.getServer());
        next(action);
        break;

      case 'SNAPCAST_DISCONNECT':
        if (socket != null) socket.close();
        socket = null;
        clearTimeout(reconnectTimer);
        break;

      case 'SNAPCAST_SET_CONNECTION':
        store.dispatch(snapcastActions.serverLoaded({}));
        store.dispatch(snapcastActions.clientsLoaded([]));
        store.dispatch(snapcastActions.groupsLoaded([]));
        store.dispatch(snapcastActions.streamsLoaded([]));
        store.dispatch(snapcastActions.set(action.data));

        // Wait 250 ms and then retry connection
        if (store.getState().snapcast.enabled) {
          setTimeout(
            () => {
              store.dispatch(snapcastActions.connect());
            },
            250,
          );
        }
        break;

      case 'SNAPCAST_DEBUG':
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

      case 'SNAPCAST_REQUEST':
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
                  'Snapcast request failed',
                  error,
                  action.method,
                  action,
                ));
              }
            },
          );
        break;

      case 'SNAPCAST_SET_ENABLED':
        store.dispatch(snapcastActions.set({ enabled: action.enabled }));
        if (!action.enabled) {
          store.dispatch(snapcastActions.set({ streaming_enabled: false }));
          store.dispatch(snapcastActions.disconnect());
        } else {
          store.dispatch(snapcastActions.connect());
        }
        break;

      case 'SNAPCAST_GET_SERVER':
        request(store, 'Server.GetStatus')
          .then(
            (response) => {
              store.dispatch(snapcastActions.serverLoaded(response.server.server, true));
              store.dispatch(snapcastActions.groupsLoaded(response.server.groups, true));
              store.dispatch(snapcastActions.streamsLoaded(response.server.streams, true));
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Could not get Snapcast server',
                error,
              ));
            },
          );
        break;

      case 'SNAPCAST_GROUPS_LOADED':
        var groups_index = { ...snapcast.groups };
        var clients_loaded = [];

        const groups_loaded = action.groups.map((raw_group) => {
          let group = formatGroup(raw_group);

          if (groups_index[group.id]) {
            group = { ...groups_index[group.id], ...group };
          }

          if (raw_group.clients) {
            group.clients_ids = arrayOf('id', raw_group.clients);
            clients_loaded = [...clients_loaded, ...raw_group.clients];
          }

          // Create a name (display only) based on it's ID
          if (group.name === undefined || group.name === '') {
            group.name = `Group ${group.id.substring(0, 3)}`;
          }

          return group;
        });

        action.groups = groups_loaded;

        if (clients_loaded.length > 0) {
          store.dispatch(snapcastActions.clientsLoaded(clients_loaded, action.flush));
        }

        next(action);
        break;

      case 'SNAPCAST_CLIENTS_LOADED': {
        const nextClients = action.flush ? {} : { ...snapcast.clients };

        for (const raw_client of action.clients) {
          const client = formatClient(raw_client);
          nextClients[client.id] = {
            ...nextClients[client.id],
            ...client,
          };
        }

        next({
          ...action,
          clients: nextClients,
        });
        break;
      }

      case 'SNAPCAST_SET_CLIENT_NAME':
        var client = snapcast.clients[action.id];
        var params = {
          id: action.id,
          name: action.name,
        };

        request(store, 'Client.SetName', params)
          .then(
            (response) => {
              store.dispatch(snapcastActions.clientLoaded(
                {
                  id: action.id,
                  name: response.name,
                },
              ));
            },
          );
        break;

      case 'SNAPCAST_SET_CLIENT_MUTE':
        var client = store.getState().snapcast.clients[action.id];
        var params = {
          id: action.id,
          volume: {
            muted: action.mute,
            percent: client.volume,
          },
        };

        request(store, 'Client.SetVolume', params)
          .then(
            (response) => {
              store.dispatch(snapcastActions.clientLoaded(
                {
                  id: action.id,
                  volume: response.volume.percent,
                  mute: response.volume.muted,
                },
              ));
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Error',
                error,
                error.message,
              ));
            },
          );
        break;

      case 'SNAPCAST_SET_CLIENT_VOLUME':
        var client = snapcast.clients[action.id];
        var params = {
          id: action.id,
          volume: {
            muted: client.mute,
            percent: action.volume,
          },
        };

        request(store, 'Client.SetVolume', params)
          .then(
            (response) => {
              store.dispatch(snapcastActions.clientLoaded(
                {
                  id: action.id,
                  volume: response.volume.percent,
                },
              ));
              /*
              // A group was referenced, so we should update the group's averaged volume
              if (action.group_id) {
                const group = snapcast.groups[action.group_id];
                const clients = [];
                snapcast.groups.filter
                store.dispatch(snapcastActions.calculateGroupVolume(group.id, clients));
              }
              */
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Error',
                error,
                error.message,
              ));
            },
          );
        break;

      case 'SNAPCAST_SET_CLIENT_LATENCY':
        var client = store.getState().snapcast.clients[action.id];
        var params = {
          id: action.id,
          latency: action.latency,
        };

        request(store, 'Client.SetLatency', params)
          .then(
            (response) => {
              store.dispatch(snapcastActions.clientLoaded(
                {
                  id: action.id,
                  latency: response.latency,
                },
              ));
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Error',
                error,
                error.message,
              ));
            },
          );
        break;

      case 'SNAPCAST_SET_CLIENT_GROUP':

        var group = snapcast.groups[action.group_id];
        var { clients_ids } = group;
        var clients_ids_index = clients_ids.indexOf(action.id);

        // Not in group (yet), so add it
        if (clients_ids_index <= -1) {
          clients_ids.push(action.id);

          // Already there, so remove it
        } else {
          clients_ids.splice(clients_ids_index, 1);
        }

        var params = {
          id: action.group_id,
          clients: clients_ids,
        };

        request(store, 'Group.SetClients', params)
          .then(
            (response) => {
              store.dispatch(snapcastActions.groupsLoaded(response.server.groups, true));
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Error',
                error,
                error.message,
              ));
            },
          );
        break;

      case 'SNAPCAST_DELETE_CLIENT':
        request(store, 'Server.DeleteClient', { id: action.id })
          .then(
            () => {
              store.dispatch({
                type: 'SNAPCAST_CLIENT_REMOVED',
                key: action.data.params.id,
              });
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Error',
                error,
                error.message,
              ));
            },
          );
        break;

      case 'SNAPCAST_SET_GROUP_NAME':
        var group = snapcast.groups[action.id];
        var params = {
          id: action.id,
          name: action.name,
        };

        request(store, 'Group.SetName', params)
          .then(
            (response) => {
              store.dispatch(snapcastActions.groupLoaded(
                {
                  id: action.id,
                  name: response.name,
                },
              ));
            },
          );
        break;

      case 'SNAPCAST_SET_GROUP_STREAM':
        var group = store.getState().snapcast.groups[action.id];
        var params = {
          id: action.id,
          stream_id: action.stream_id,
        };

        request(store, 'Group.SetStream', params)
          .then(
            (response) => {
              store.dispatch(snapcastActions.groupLoaded(
                {
                  id: action.id,
                  stream_id: action.stream_id,
                },
              ));
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Could not change stream',
                error,
                error.message,
              ));
            },
          );
        break;

      case 'SNAPCAST_SET_GROUP_MUTE':
        var group = store.getState().snapcast.groups[action.id];
        var params = {
          id: action.id,
          mute: action.mute,
        };

        request(store, 'Group.SetMute', params)
          .then(
            (response) => {
              store.dispatch(snapcastActions.groupLoaded(
                {
                  id: action.id,
                  mute: response.mute,
                },
              ));
            },
            (error) => {
              store.dispatch(coreActions.handleException(
                'Could not toggle mute',
                error,
                error.message,
              ));
            },
          );
        break;

      case 'SNAPCAST_SET_GROUP_VOLUME':
        var clients_to_update = [];
        var group = snapcast.groups[action.id];
        var change = action.percent - action.old_percent;

        for (const client_id of group.clients_ids) {
          // Apply the change proportionately to each client
          var client = snapcast.clients[client_id];
          const current_volume = client.volume;
          const new_volume = current_volume + change;

          // Only change if the client is within min/max limits
          if ((change > 0 && current_volume < 100) || (change < 0 && current_volume > 0)) {
            clients_to_update.push({
              id: client.id,
              volume: new_volume,
            });
          }
        }

        // Loop our required changes, and post each to Snapcast
        for (const client_to_update of clients_to_update) {
          let volume = client_to_update.volume + ((group.clients_ids.length - clients_to_update.length) * change);

          // Make sure we're not creating an impossible percent
          if (volume < 0) {
            volume = 0;
          } else if (volume > 100) {
            volume = 100;
          }

          store.dispatch(snapcastActions.setClientVolume(client_to_update.id, volume));
        }

        store.dispatch(snapcastActions.groupLoaded({
          id: action.id,
          volume: action.percent,
        }));
        break;

      default:
        return next(action);
    }
  };
}());

export default SnapcastMiddleware;


import ReactGA from 'react-ga';

const helpers = require('../../helpers');
const coreActions = require('../core/actions');
const uiActions = require('../ui/actions');
const pusherActions = require('../pusher/actions');
const snapcastActions = require('./actions');

const SnapcastMiddleware = (function () {
  let socket = null;

  // requests pending
  const deferredRequests = [];

  // handle all manner of socket messages
  const handleMessage = (ws, store, message) => {
    if (store.getState().ui.log_snapcast) {
      console.log('Snapcast log (incoming)', message);
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
      // Broadcast of an error
      if (message.error !== undefined) {
        store.dispatch(coreActions.handleException(
          `Snapcast: ${message.error.message}`,
          message,
          (message.error.data !== undefined && message.error.data.description !== undefined ? message.error.data.description : null),
        ));
      } else {
        switch (message.method) {
          case 'connection_added':
            store.dispatch(pusherActions.connectionAdded(message.params.connection));
            break;
        }
      }
    }
  };

  const request = (store, method, params = null) => new Promise((resolve, reject) => {
    const id = helpers.generateGuid();
    const message = {
      jsonrpc: '2.0',
      id,
      method,
    };
    if (params) {
      message.params = params;
    }

    if (store.getState().ui.log_pusher) {
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
        if (socket != null) {
          socket.close();
        }

        store.dispatch({ type: 'SNAPCAST_CONNECTING' });
        var state = store.getState();

        socket = new Mopidy({
          webSocketUrl: `ws${window.location.protocol === 'https:' ? 's' : ''}://${state.snapcast.host}:${state.snapcast.port}/jsonrpc/`,
          callingConvention: 'by-position-or-by-name',
        });

        socket.on((type, data) => handleMessage(socket, store, type, data));
        break;

      case 'SNAPCAST_CONNECTED':
        if (store.getState().ui.allow_reporting) {
          const hashed_hostname = sha256(window.location.hostname);
          ReactGA.event({ category: 'Snapcast', action: 'Connected', label: hashed_hostname });
        }
        store.dispatch(uiActions.createNotification({ content: 'Snapcast connected' }));
        next(action);
        break;

      case 'SNAPCAST_DISCONNECT':
        if (socket != null) socket.close();
        socket = null;
        store.dispatch({ type: 'SNAPCAST_DISCONNECTED' });
        break;

      case 'SNAPCAST_DISCONNECTED':
        store.dispatch(uiActions.createNotification({ type: 'bad', content: 'Snapcast disconnected' }));
        helpers.setFavicon('favicon_error.png');
        break;

      case 'SNAPCAST_DEBUG':
        request(socket, store, action.call, action.value)
          .then((response) => {
            store.dispatch({ type: 'DEBUG', response });
          });
        break;


      case 'SNAPCAST_GET_SERVER':
        request(
	                store,
	                {
	                	method: 'Server.GetStatus',
	                },
	                (response) => {
            store.dispatch(snapcastActions.serverLoaded(response.server));
	                },
	                (error) => {
            store.dispatch(coreActions.handleException(
              'Could not get server',
              error,
            ));
	                },
	            );
        break;

      case 'SNAPCAST_SERVER_LOADED':
        store.dispatch(snapcastActions.groupsLoaded(action.server.groups, true));
        store.dispatch(snapcastActions.streamsLoaded(action.server.streams, true));

        // Snapcast double-nests the server object
        action.server = action.server.server;

        next(action);
        break;

      case 'SNAPCAST_GROUPS_LOADED':
        var groups_index = { ...snapcast.groups };
        var groups_loaded = [];
        var clients_loaded = [];

        for (const raw_group of action.groups) {
          var group = helpers.formatGroup(raw_group);

          if (groups_index[group.id]) {
            group = { ...groups_index[group.id], ...group };
          }

          if (raw_group.clients) {
            group.clients_ids = helpers.arrayOf('id', raw_group.clients);
            clients_loaded = [...clients_loaded, ...raw_group.clients];
          }

          groups_loaded.push(group);
        }

        action.groups = groups_loaded;

        if (clients_loaded.length > 0) {
          store.dispatch(snapcastActions.clientsLoaded(clients_loaded, action.flush));
        }

        next(action);
        break;

      case 'SNAPCAST_CLIENTS_LOADED':
        var clients_index = { ...snapcast.clients };
        var clients_loaded = [];

        for (const raw_client of action.clients) {
          var client = helpers.formatClient(raw_client);

          if (clients_index[client.id]) {
            client = { ...clients_index[client.id], ...client };
          }

          clients_loaded.push(client);
        }

        action.clients = clients_loaded;

        next(action);
        break;

      case 'SNAPCAST_SET_CLIENT_NAME':
        var client = snapcast.clients[action.id];
        var data = {
          method: 'Client.SetName',
          params: {
            id: action.id,
            name: action.name,
          },
        };

        request(
                	store,
                	data,
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
        var data = {
          method: 'Client.SetVolume',
          params: {
            id: action.id,
            volume: {
              muted: action.mute,
              percent: client.volume,
            },
          },
        };

        request(
                	store,
                	data,
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

        var data = {
          method: 'Client.SetVolume',
          params: {
            id: action.id,
            volume: {
              muted: client.mute,
              percent: action.volume,
            },
          },
        };

        request(
                	store,
                	data,
          (response) => {
            store.dispatch(snapcastActions.clientLoaded(
              {
                id: action.id,
                volume: response.volume.percent,
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

      case 'SNAPCAST_SET_CLIENT_LATENCY':
        var client = store.getState().snapcast.clients[action.id];
        var data = {
          method: 'Client.SetLatency',
          params: {
            id: action.id,
            latency: action.latency,
          },
        };

        request(
          store,
          data,
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

        var data = {
          method: 'Group.SetClients',
          params: {
            id: action.group_id,
            clients: clients_ids,
          },
        };

        request(
                	store,
                	data,
          (response) => {
            store.dispatch(snapcastActions.serverLoaded(response.server));
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
        var data = {
          method: 'Server.DeleteClient',
          params: {
            id: action.id,
          },
        };

        request(
                	store,
                	data,
          (response) => {
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

      case 'SNAPCAST_SET_GROUP_STREAM':
        var group = store.getState().snapcast.groups[action.id];
        var data = {
          method: 'Group.SetStream',
          params: {
            id: action.id,
            stream_id: action.stream_id,
          },
        };

        request(
                	store,
                	data,
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
        var data = {
          method: 'Group.SetMute',
          params: {
            id: action.id,
            mute: action.mute,
          },
        };

        request(
                	store,
                	data,
          (response) => {
            store.dispatch(snapcastActions.groupLoaded(
              {
                id: action.id,
                muted: response.mute,
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
        break;

      case 'SNAPCAST_EVENT_RECEIVED':

        // Drop the prefix
        action.method = action.method.replace('snapcast_', '');

        switch (action.method) {
          case 'Client.OnConnect':
            store.dispatch(snapcastActions.clientLoaded(
              {
                id: action.params.client.id,
                name: action.params.client.name,
                volume: action.params.client.volume.percent,
                mute: action.params.client.volume.muted,
                connected: action.params.client.connected,
              },
            ));
            break;

          case 'Client.OnDisconnect':
            store.dispatch(snapcastActions.clientLoaded(
              {
                id: action.params.client.id,
                connected: action.params.client.connected,
              },
            ));
            break;

          case 'Client.OnVolumeChanged':
            store.dispatch(snapcastActions.clientLoaded(
              {
                id: action.params.id,
                mute: action.params.volume.muted,
                volume: action.params.volume.percent,
              },
            ));
            break;

          case 'Client.OnLatencyChanged':
            store.dispatch(snapcastActions.clientLoaded(
              {
                id: action.params.id,
                latency: action.params.latency,
              },
            ));
            break;

          case 'Client.OnNameChanged':
            store.dispatch(snapcastActions.clientLoaded(
              {
                id: action.params.id,
                name: action.params.name,
              },
            ));
            break;

          case 'Group.OnMute':
            store.dispatch(snapcastActions.groupLoaded(
              {
                id: action.params.id,
                mute: action.params.mute,
              },
            ));
            break;

          case 'Server.OnUpdate':
            store.dispatch(snapcastActions.serverLoaded(action.params));
            break;
        }
        break;

        // This action is irrelevant to us, pass it on to the next middleware
      default:
        return next(action);
    }
  };
}());

export default SnapcastMiddleware;

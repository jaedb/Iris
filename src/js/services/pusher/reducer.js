
export default function reducer(pusher = {}, action) {
  switch (action.type) {
    case 'PUSHER_CONNECT':
    case 'PUSHER_CONNECTING':
      return { ...pusher, connected: false, connecting: true };

    case 'PUSHER_CONNECTED':
      return {
        ...pusher,
        connected: true,
        connecting: false,
        connection_id: action.connection_id,
        client_id: action.client_id,
        username: action.username,
      };

    case 'PUSHER_DISCONNECTED':
      return { ...pusher, connected: false, connecting: false };

    case 'PUSHER_SET_PORT':
      return { ...pusher, port: action.port };

    case 'PUSHER_SET_USERNAME':
      return { ...pusher, username: action.username };

    case 'PUSHER_CONNECTIONS':
      var connections = {};
      for (let i = 0; i < action.connections.length; i++) {
        connections[action.connections[i].connection_id] = action.connections[i];
      }
      return { ...pusher, connections };

    case 'PUSHER_CONNECTION_ADDED':
    case 'PUSHER_CONNECTION_CHANGED':
      var connections = { ...pusher.connections };
      connections[action.connection.connection_id] = action.connection;
      return { ...pusher, connections };

    case 'PUSHER_CONNECTION_UPDATED':
      return {
        ...pusher,
        username: action.connection.username,
        client_id: action.connection.client_id,
        connection_id: action.connection.connection_id,
      };

    case 'PUSHER_CONNECTION_REMOVED':
      var connections = { ...pusher.connections };
      delete connections[action.connection.connection_id];
      return { ...pusher, connections };

    case 'PUSHER_VERSION':
      return {
        ...pusher,
        version: action.version,
        upgrading: false,
      };

    case 'PUSHER_START_UPGRADE':
      return { ...pusher, upgrading: true };

    case 'PUSHER_CONFIG':
      return { ...pusher, config: action.config };

    case 'PUSHER_PINNED_UPDATED':
      return { ...pusher, pinned: action.pinned };

    case 'PUSHER_COMMANDS_UPDATED':
      return { ...pusher, commands: action.commands };

    default:
      return pusher;
  }
}


export default function reducer(pusher = {}, action){
    switch (action.type) {

        case 'PUSHER_CONNECT':
        case 'PUSHER_CONNECTING':
            return Object.assign({}, pusher, { connected: false, connecting: true });

        case 'PUSHER_CONNECTED':
            return Object.assign({}, pusher, {
                connected: true,
                connecting: false,
                connection_id: action.connection_id,
                username: action.username
            });

        case 'PUSHER_DISCONNECTED':
            return Object.assign({}, pusher, { connected: false, connecting: false });

        case 'PUSHER_SET_PORT':
            return Object.assign({}, pusher, { port: action.port });

        case 'PUSHER_USERNAME_CHANGED':
            return Object.assign({}, pusher, { username: action.username });

        case 'PUSHER_CONNECTIONS':
            var connections = {}
            for (var i = 0; i < action.connections.length; i++){
                connections[action.connections[i].connection_id] = action.connections[i]
            }
            return Object.assign({}, pusher, { connections: connections });

        case 'PUSHER_CONNECTION_ADDED':
        case 'PUSHER_CONNECTION_UPDATED':
            var connections = Object.assign({}, pusher.connections)
            connections[action.connection.connection_id] = action.connection
            return Object.assign({}, pusher, { connections: connections });

        case 'PUSHER_CONNECTION_REMOVED':
            var connections = Object.assign({}, pusher.connections)
            delete connections[action.connection.connection_id]
            return Object.assign({}, pusher, { connections: connections });

        case 'PUSHER_VERSION':
            return Object.assign({}, pusher, { 
                version: action.version,
                upgrading: false
            });

        case 'PUSHER_START_UPGRADE':
            return Object.assign({}, pusher, { upgrading: true });

        default:
            return pusher
    }
}




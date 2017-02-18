
export default function reducer(pusher = {}, action){
    switch (action.type) {

        case 'PUSHER_CONNECT':
        case 'PUSHER_CONNECTING':
            return Object.assign({}, pusher, { connected: false, connecting: true });

        case 'PUSHER_CONNECTED':
            return Object.assign({}, pusher, { 
                connected: true, 
                connecting: false, 
                connection_id: action.connection.connection_id, 
                username: action.connection.username
            });

        case 'PUSHER_DISCONNECTED':
            return Object.assign({}, pusher, { connected: false, connecting: false });

        case 'PUSHER_SET_PORT':
            return Object.assign({}, pusher, { port: action.port });

        case 'PUSHER_USERNAME':
            return Object.assign({}, pusher, { username: action.username });

        case 'CONNECTIONS':
            return Object.assign({}, pusher, { connections: action.connections });

        case 'CONNECTION_UPDATED':
            function byID(connection){
                return connection.connection_id == action.connection.connection_id;
            }
            var connection = pusher.connections.find(byID);
            var index = pusher.connections.indexOf(connection);
            var connections = Object.assign([], pusher.connections);
            connections[index] = action.connection;

            return Object.assign({}, pusher, { connections: connections });

        case 'VERSION':
            return Object.assign({}, pusher, { 
                version: action.version,
                upgrading: false
            });

        case 'START_UPGRADE':
            return Object.assign({}, pusher, { upgrading: true });

        default:
            return pusher
    }
}




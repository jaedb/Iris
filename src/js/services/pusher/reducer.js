
export default function reducer(pusher = {}, action){
    switch (action.type) {

        case 'PUSHER_CONNECTED':
            return Object.assign({}, pusher, { connected: true, connecting: false, connectionid: action.connection.connectionid });

        case 'PUSHER_DISCONNECTED':
            return Object.assign({}, pusher, { connected: false, connecting: false });

        case 'PUSHER_SET_CONFIG':
            return Object.assign({}, pusher, { 
                username: action.username,
                port: action.port
            });

        case 'PUSHER_CONNECTION_UPDATED':
            function byID(connection){
                return connection.connectionid == action.data.connections.connectionid;
            }
            var connection = pusher.connections.find(byID);
            var index = pusher.connections.indexOf(connection);
            var connections = Object.assign([], pusher.connections);
            connections[index] = action.data.connections;

            return Object.assign({}, pusher, { connections: connections });

        case 'PUSHER_CONNECTIONS':
            return Object.assign({}, pusher, { connections: action.data.connections });

        case 'PUSHER_VERSION':
            return Object.assign({}, pusher, { version: action.data });

        default:
            return pusher
    }
}





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

        case 'PUSHER_CLIENT_CONNECTED':
            return Object.assign({}, pusher, { connection: action.data });

        case 'PUSHER_CONNECTIONS':
            return Object.assign({}, pusher, { connections: action.data.connections });

        case 'PUSHER_VERSION':
            return Object.assign({}, pusher, { version: action.data });

        default:
            return pusher
    }
}




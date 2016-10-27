
export default function reducer(pusher = {}, action){
    switch (action.type) {

        case 'PUSHER_CONNECTED':
            return Object.assign({}, pusher, { connected: true, connecting: false });

        case 'PUSHER_DISCONNECTED':
            return Object.assign({}, pusher, { connected: false, connecting: false });

        case 'PUSHER_SET_PORT':
            return Object.assign({}, pusher, { port: action.port });

        case 'PUSHER_CLIENT_CONNECTED':
            return Object.assign({}, pusher, { connections: action.data });

        case 'PUSHER_CONNECTIONS_LOADED':
            return Object.assign({}, pusher, { connections: action.data });

        case 'PUSHER_VERSION':
            return Object.assign({}, pusher, { version: action.data });

        default:
            return pusher
    }
}




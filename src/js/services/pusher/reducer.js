
export default function reducer(pusher = {}, action){
    switch (action.type) {

        case 'PUSHER_CONNECTED':
            return Object.assign({}, pusher, { connected: true, connecting: false });

        case 'PUSHER_DISCONNECTED':
            return Object.assign({}, pusher, { connected: false, connecting: false });

        case 'PUSHER_SET_CONFIG':
            return Object.assign({}, pusher, {
                host: action.host, 
                port: action.port
            });

        default:
            return pusher
    }
}




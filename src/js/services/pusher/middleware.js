
var actions = require('./actions.js')
var helpers = require('../../helpers.js')

const PusherMiddleware = (function(){ 

    // container for the actual Mopidy socket
    var socket = null;

    // handle all manner of socket messages
    const handleMessage = (ws, store, message) => {
        switch( message.action ){
            default:
                var name = message.action.toUpperCase();
                name = name.replace('GET_','');
                store.dispatch({ type: 'PUSHER_'+name, data: message.data })
        }
    }

    const makeRequest = (data) => {
        data.type = 'query';
        data.message_id = helpers.generateGuid();
        socket.send( JSON.stringify(data) );
    }

    /**
     * Middleware
     *
     * This behaves like an action interceptor. We listen for specific actions
     * and handle special functionality. If the action is not in our switch, then
     * it just proceeds to the next middleware, or default functionality
     **/
    return store => next => action => {

        switch(action.type) {

            case 'PUSHER_CONNECT':

                if(socket != null) socket.close();
                store.dispatch({ type: 'PUSHER_CONNECTING' });

                var state = store.getState();
                var connection = {
                    clientid: Math.random().toString(36).substr(2, 9),
                    connectionid: helpers.generateGuid(),
                    username: Math.random().toString(36).substr(2, 9)
                }
                if( state.pusher.username ) connection.username = state.pusher.username;

                socket = new WebSocket(
                    'ws://'+state.mopidy.host+':'+state.pusher.port+'/pusher',
                    [ connection.clientid, connection.connectionid, connection.username ]
                );

                socket.onopen = () => {
                    store.dispatch({ type: 'PUSHER_CONNECTED', connection: connection });
                    store.dispatch({ type: 'PUSHER_SET_USERNAME', username: connection.username });
                };

                socket.onmessage = (message) => {
                    var message = JSON.parse(message.data);
                    handleMessage( socket, store, message )
                };

                break;

            case 'PUSHER_CONNECTED':
                makeRequest({ action: 'get_version' });
                return next(action);
                break;

            case 'PUSHER_UPGRADING':
                makeRequest({ action: 'perform_upgrade' });
                return next(action);
                break;

            case 'PUSHER_CLIENT_CONNECTED':
                makeRequest({ action: 'get_connections' });
                return next(action);
                break;

            case 'PUSHER_INSTRUCT':
                makeRequest({ action: action.action, data: action.data });
                break;

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default PusherMiddleware
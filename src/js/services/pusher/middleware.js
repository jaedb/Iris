
var actions = require('./actions.js')

const PusherMiddleware = (function(){ 

    // container for the actual Mopidy socket
    var socket = null;

    // handle all manner of socket messages
    const handleMessage = (ws, store, message) => {
        switch( message.action ){
            default:
                store.dispatch({ type: 'PUSHER_'+message.action.toUpperCase(), data: message.data })
        }
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

                socket = new WebSocket(
                    'ws://'+state.mopidy.host+':'+state.pusher.port+'/pusher'
                );

                socket.onopen = function(){
                    store.dispatch({ type: 'PUSHER_CONNECTED' });
                };

                socket.onmessage = function(message){
                    var message = JSON.parse(message.data);
                    handleMessage( socket, store, message )
                };

                break;

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default PusherMiddleware
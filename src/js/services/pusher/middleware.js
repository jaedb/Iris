
var pusherActions = require('./actions.js')
var uiActions = require('../ui/actions.js')
var helpers = require('../../helpers.js')

const PusherMiddleware = (function(){ 

    // container for the actual Mopidy socket
    var socket = null;

    // handle all manner of socket messages
    const handleMessage = (ws, store, message) => {
        switch( message.action ){
            default:
                var name = 'unspecified'
                if( message.action ) name = message.action
                name = name.replace('get_','').toUpperCase()
                store.dispatch({ type: 'PUSHER_'+name, data: message.data })
        }
    }

    const makeRequest = (data) => {
        data.type = 'query';
        data.message_id = helpers.generateGuid();
        socket.send( JSON.stringify(data) );
    }

    const broadcast = (data) => {
        data.type = 'broadcast';
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
                switch( action.message_type ){
                    case 'query':
                        makeRequest( action.data )
                        break
                    case 'broadcast':
                        broadcast( action.data )
                        break
                }
                break;

            case 'PUSHER_DEBUG':
                switch( action.call ){
                    case 'query':
                        makeRequest( action.data )
                            // THIS DOES NOT RETURN A PROMISE SO CAN'T DETECT RELATED RESPONSES
                            /*.then( response => {
                                store.dispatch({ type: 'DEBUG', response: response })
                            })*/
                        break
                    case 'broadcast':
                        broadcast( action.data )
                            /*.then( response => {
                                store.dispatch({ type: 'DEBUG', response: response })
                            })*/
                        break
                    default:
                        store.dispatch({ type: 'DEBUG', response: '{ "error": "Invalid call" }' })
                        break
                }
                break;

            case 'PUSHER_SEND_BROADCAST':
                broadcast({ action: action.action, data: action.data });
                break;

            case 'PUSHER_NOTIFICATION':
            case 'PUSHER_BROADCAST':

                var notification = window.Notification || window.mozNotification || window.webkitNotification;
                if ('undefined' === typeof notification) return false;
                if ('undefined' !== typeof notification) notification.requestPermission(function(permission){});

                // handle nested data objects
                var data = {}
                if( typeof(action.data) ) data = action.data
                if( typeof(data.data) ) data = Object.assign({}, data, data.data)

                // construct our browser notification
                var title = '';
                var options = {
                    body: '',
                    dir: 'auto',
                    lang: 'EN',
                    tag: 'iris'
                };
                if( data.title ) title = data.title;
                if( data.body ) options.body = data.body;
                if( data.icon ) options.icon = data.icon;

                // make it so
                var notification = new notification( title, options );
                break;

            case 'PUSHER_SEND_AUTHORIZATION':
                if( window.confirm('Spotify authorization for user '+action.data.me.id+' received. Do you want to import?') ){

                    // remove any existing authentication
                    store.dispatch({ type: 'SPOTIFY_AUTHORIZATION_REVOKED' })

                    // import our new authentication
                    store.dispatch({ type: 'SPOTIFY_ME_LOADED', data: action.data.me })
                    store.dispatch({ type: 'SPOTIFY_AUTHORIZATION_GRANTED', data: action.data.authorization })
                }else{
                    console.log('Authorization ignored')
                }
                break

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default PusherMiddleware
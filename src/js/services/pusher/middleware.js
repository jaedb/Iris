
var pusherActions = require('./actions.js')
var uiActions = require('../ui/actions.js')
var helpers = require('../../helpers.js')

const PusherMiddleware = (function(){ 

    // container for the actual Mopidy socket
    var socket = null;
    var deferredRequests = [];

    const resolveRequest = (requestId, message ) => {
        var response = JSON.parse( message );
        deferredRequests[request_id].resolve( response );
        delete deferredRequests[request_id];
    }

    const rejectRequest = (requestId, message) => {
        deferredRequests[requestId].reject( message );
    }

    // handle all manner of socket messages
    const handleMessage = (ws, store, message) => {

        //console.log('handleMessage', message)

        switch (message.action){
            case 'response':
                if (typeof( deferredRequests[ message.request_id ]) !== 'undefined' ){
                    deferredRequests[ message.request_id ].resolve( message )
                } else {
                    console.error('Pusher: Response with no matching request', message);
                }
                break

            case 'broadcast':                
                if (message.type ){
                    var type = message.type.toUpperCase()
                } else if (message.data.type ){
                    var type = message.data.type.toUpperCase()
                } else {
                    var type = 'UNRECOGNISED_BROADCAST'
                }
                store.dispatch({ type: type, data: message.data })
                break
        }
    }

    const request = (data) => {
        return new Promise( (resolve, reject) => {

            // send the payload
            data.request_id = helpers.generateGuid()
            socket.send( JSON.stringify(data) )
            
            // add query to our deferred responses
            deferredRequests[ data.request_id ] = {
                resolve: resolve,
                reject: reject
            }
        })
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

            case 'PUSHER_INSTRUCT':
                request( action.data )
                    .then(
                        response => {
                            store.dispatch({ type: 'PUSHER_INSTRUCT', data: response.data })
                        }
                    )
                break

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
                    request({ action: 'get_radio' })
                        .then(
                            response => {
                                store.dispatch({ type: 'RADIO', data: response.data })
                            }
                        )
                };

                socket.onmessage = (message) => {
                    var message = JSON.parse(message.data);
                    handleMessage( socket, store, message )
                };

                break;

            case 'PUSHER_CONNECTED':
                request({ action: 'get_version' })
                    .then(
                        response => {
                            store.dispatch({ type: 'VERSION', data: response.data })
                        }
                    )
                return next(action);
                break;

            case 'START_UPGRADE':
                request({ action: 'upgrade' })
                    .then(
                        response => {
                            if (response.data.upgrade_successful){
                                store.dispatch( uiActions.createNotification('Upgrade complete') )
                            }else{
                                store.dispatch( uiActions.createNotification('Upgrade failed, please upgrade manually','bad') )
                            }
                            store.dispatch({ type: 'VERSION', data: response.data })
                        }
                    )
                return next(action);
                break;

            case 'PUSHER_SET_USERNAME':
                request({ action: 'set_username', username: action.username })
                    .then(
                        response => {
                            store.dispatch({ type: 'PUSHER_USERNAME', data: { username: response.data.username }})
                        }
                    )
                return next(action);
                break;

            case 'GET_CONNECTIONS':
            case 'NEW_CONNECTION':
                request({ action: 'get_connections' })
                    .then(
                        response => {                            
                            store.dispatch({ type: 'CONNECTIONS', data: response.data })
                        }
                    )
                return next(action);
                break

            case 'PUSHER_DEBUG':
                request( action.data )
                    .then(
                        response => {                            
                            store.dispatch({ type: 'DEBUG', response: response.data })
                        }
                    )
                break;

            case 'PUSHER_SEND_AUTHORIZATION':
                request({
                    action: 'send_authorization',
                    recipient_connectionid: action.recipient_connectionid,
                    authorization: action.authorization,
                    me: action.me
                })
                .then(
                    response => {                   
                        store.dispatch( uiActions.createNotification('Authorization sent') )
                    }
                )
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

            case 'START_RADIO':

                request({
                    action: 'broadcast',
                    data: {
                        type: 'browser_notification',
                        title: 'Radio started',
                        body: store.getState().pusher.username +' started radio mode',
                        icon: ''
                    }
                })
                .then(
                    response => {                   
                        store.dispatch(uiActions.createNotification('Starting radio...'))
                    }
                )

                var data = {
                    action: 'start_radio',
                    seed_artists: [],
                    seed_genres: [],
                    seed_tracks: []
                }
                
                for( var i = 0; i < action.uris.length; i++){
                    switch( helpers.uriType( action.uris[i] ) ){
                        case 'artist':
                            data.seed_artists.push( action.uris[i] );
                            break;
                        case 'track':
                            data.seed_tracks.push( action.uris[i] );
                            break;
                    }
                }
                
                request( data )
                break

            case 'STOP_RADIO':

                request({
                    action: 'broadcast',
                    data: {
                        type: 'browser_notification',
                        title: 'Radio stopped',
                        body: store.getState().pusher.username +' stopped radio mode',
                        icon: ''
                    }
                })
                .then(
                    response => {                   
                        store.dispatch(uiActions.createNotification('Stopping radio'))
                    }
                )

                store.dispatch(uiActions.createNotification('Stopping radio'))
                var data = {
                    action: 'stop_radio',
                    seed_artists: [],
                    seed_genres: [],
                    seed_tracks: []
                }
                request( data )
                break

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default PusherMiddleware

var helpers = require('../../helpers.js')
var uiActions = require('../ui/actions.js')
var pusherActions = require('./actions.js')
var spotifyActions = require('../spotify/actions.js')

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
        console.log(message)
        // response to a request [we] made
        if (typeof(message.request_id) !== 'undefined' && message.request_id){            
            if (typeof( deferredRequests[ message.request_id ]) !== 'undefined' ){
                deferredRequests[ message.request_id ].resolve( message )
            } else {
                console.error('Pusher: Response with no matching request', message);
            }

        // general message
        // this can be client-client, server-client or a broadcast to many clients
        } else {
            message.type = message.type.toUpperCase()
            store.dispatch(message)
        }
    }

    const request = (method, data = {}) => {
        return new Promise( (resolve, reject) => {
            var request_id = helpers.generateGuid()
            var message = {
                method: method,
                data: data,
                request_id: request_id
            }
            socket.send( JSON.stringify(message) )
            
            // add query to our deferred responses
            deferredRequests[request_id] = {
                resolve: resolve,
                reject: reject
            }
        })
    }

    return store => next => action => {
        switch(action.type) {

            case 'PUSHER_INSTRUCT':
                request( action )
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
                    clientid: helpers.generateGuid(),
                    connection_id: helpers.generateGuid(),
                    username: 'Anonymous'
                }
                if( state.pusher.username ) connection.username = state.pusher.username;
                connection.username = connection.username.replace(/\W/g, '')
                
                socket = new WebSocket(
                    'ws://'+state.mopidy.host+':'+state.mopidy.port+'/iris/ws',
                    [ connection.clientid, connection.connection_id, connection.username ]
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
                request('get_config')
                    .then(
                        response => {
                            if (response.error){
                                console.error(response.error)
                                return false
                            }

                            response.type = 'CONFIG'
                            store.dispatch(response)
                            if (response.config.spotify_username){
                                store.dispatch(spotifyActions.getUser('spotify:user:'+response.config.spotify_username))
                            }
                            var spotify = store.getState().spotify
                            if (!spotify.country || !spotify.locale){
                                store.dispatch({ type: 'SPOTIFY_SET_CONFIG', config: response.config })
                            }
                        }
                    )
                request('get_version')
                    .then(
                        response => {
                            if (response.error){
                                console.error(response.error)
                                return false
                            }
                            response.type = 'VERSION'
                            store.dispatch(response)
                        }
                    )
                request('get_radio')
                    .then(
                        response => {
                            if (response.error){
                                console.error(response.error)
                                return false
                            }

                            response.type = 'RADIO'
                            store.dispatch(response)
                        }
                    )

                store.dispatch(pusherActions.getQueueMetadata())

                return next(action);
                break;

            case 'ERROR':
                store.dispatch( uiActions.createNotification(action.source+': '+action.message,'bad') )
                break;

            case 'PUSHER_GET_QUEUE_METADATA':
                request('get_queue_metadata')
                    .then(
                        response => {
                            response.type = 'QUEUE_METADATA'
                            store.dispatch(response)
                        }
                    )
                break;

            case 'PUSHER_ADD_QUEUE_METADATA':
                request('add_queue_metadata', {
                    tlids: action.tlids, 
                    added_from: action.from_uri,
                    added_by: store.getState().pusher.username
                })
                break;

            case 'START_UPGRADE':
                request('upgrade')
                .then(
                    response => {
                        if (response.error){
                            console.error(response.error)
                            return false
                        }

                        if (response.upgrade_successful){
                            store.dispatch( uiActions.createNotification('Upgrade complete') )
                        }else{
                            store.dispatch( uiActions.createNotification('Upgrade failed, please upgrade manually','bad') )
                        }

                        response.type = 'VERSION'
                        store.dispatch(response)
                    }
                )
                return next(action);
                break;

            case 'PUSHER_SET_USERNAME':
                request('set_username', {
                    username: action.username
                })
                .then(
                    response => {
                        console.log(response)
                        if (response.error){
                            console.error(response.error)
                            return false
                        }

                        //response.type = 'PUSHER_USERNAME'
                        //store.dispatch(response)
                    }
                )
                return next(action);
                break;

            case 'GET_CONNECTIONS':
            case 'NEW_CONNECTION':
                request('get_connections')
                .then(
                    response => {             
                        if (response.error){
                            console.error(response.error)
                            return false
                        }

                        response.type = 'CONNECTIONS'
                        store.dispatch(response)
                    }
                )
                return next(action);
                break

            case 'PUSHER_DEBUG':
                request( action )
                .then(
                    response => {          
                        if (response.error){
                            console.error(response.error)
                            return false
                        }

                        response.type = 'DEBUG'
                        store.dispatch(response)
                    }
                )
                break;

            case 'PUSHER_SEND_AUTHORIZATION':
                request('send_authorization', {
                    recipient_connection_id: action.recipient_connection_id,
                    authorization: action.authorization,
                    me: action.me
                })
                .then(
                    response => {                   
                        if (response.error){
                            console.error(response.error)
                            return false
                        }
                            
                        store.dispatch( uiActions.createNotification('Authorization sent') )
                    }
                )
                break;

            case 'PUSHER_SEND_AUTHORIZATION':
                if( window.confirm('Spotify authorization for user '+action.me.id+' received. Do you want to import?') ){

                    // remove any existing authentication
                    store.dispatch({ type: 'SPOTIFY_AUTHORIZATION_REVOKED' })

                    // import our new authentication
                    store.dispatch({ type: 'SPOTIFY_ME_LOADED', data: action.me })
                    store.dispatch({ type: 'SPOTIFY_AUTHORIZATION_GRANTED', data: action.authorization })
                }else{
                    console.log('Authorization ignored')
                }
                break

            case 'START_RADIO':
                request('broadcast', {
                    type: 'browser_notification',
                    title: 'Radio started',
                    body: store.getState().pusher.username +' started radio mode',
                    icon: ''
                })
                .then(
                    response => {                   
                        if (response.data.error){
                            console.error(response.data.error)
                            return false
                        }
                            
                        store.dispatch(uiActions.createNotification('Starting radio...'))
                    }
                )

                var data = {
                    method: 'start_radio',
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
                        case 'genre':
                            data.seed_genres.push( action.uris[i] );
                            break;
                    }
                }
                
                request( data )
                break

            case 'STOP_RADIO':
                request('broadcast', {
                    type: 'browser_notification',
                    title: 'Radio stopped',
                    body: store.getState().pusher.username +' stopped radio mode',
                    icon: ''
                })
                .then(
                    response => {           
                        if (response.data.error){
                            console.error(response.data.error)
                            return false
                        }
                                    
                        store.dispatch(uiActions.createNotification('Stopping radio'))
                    }
                )

                var data = {
                    method: 'stop_radio',
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
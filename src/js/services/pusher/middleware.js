
import ReactGA from 'react-ga'

var helpers = require('../../helpers.js')
var coreActions = require('../core/actions.js')
var uiActions = require('../ui/actions.js')
var pusherActions = require('./actions.js')
var spotifyActions = require('../spotify/actions.js')

const PusherMiddleware = (function(){ 

    // container for the actual websocket
    var socket = null

    // requests pending
    var deferredRequests = []

    // handle all manner of socket messages
    const handleMessage = (ws, store, message) => {

        // if debug enabled
        if (store.getState().ui.log_pusher) console.log('Pusher', message)

        // error
        if (message.status == 0){
            store.dispatch(coreActions.handleException(
                'Pusher: '+message.message, 
                message
            ));
        }

        // response to a request [we] made
        if (message.request_id !== undefined && message.request_id){            
            if (typeof( deferredRequests[ message.request_id ]) !== 'undefined' ){
                store.dispatch(uiActions.stopLoading(message.request_id))
                deferredRequests[ message.request_id ].resolve( message )

            } else {
                store.dispatch(coreActions.handleException(
                    'Pusher: Response received with no matching request', 
                    message
                ));
            }

        // general message
        // this can be client-client, server-client or a broadcast to many clients
        } else {
            message.type = 'PUSHER_'+message.type.toUpperCase()
            store.dispatch(message)
        }
    }

    const request = (store, method, data = {}) => {
        return new Promise( (resolve, reject) => {
            var request_id = helpers.generateGuid()
            var message = {
                method: method,
                data: data,
                request_id: request_id
            }
            socket.send( JSON.stringify(message) )

            store.dispatch(uiActions.startLoading(request_id, 'pusher_'+method))
            
            // add query to our deferred responses
            deferredRequests[request_id] = {
                resolve: resolve,
                reject: reject
            }
        })
    }

    return store => next => action => {
        switch(action.type) {

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
                    'ws'+(window.location.protocol === 'https:' ? 's' : '')+'://'+state.mopidy.host+':'+state.mopidy.port+'/iris/ws/',
                    [ connection.clientid, connection.connection_id, connection.username ]
                );

                socket.onmessage = (message) => {
                    var message = JSON.parse(message.data);
                    handleMessage( socket, store, message )
                };

                socket.onclose = () => {
                    store.dispatch({
                        type: 'PUSHER_DISCONNECTED'
                    })

                    // attempt to reconnect ever 5 seconds
                    setTimeout(() => {
                        store.dispatch(pusherActions.connect())
                    }, 5000);
                };

                break;

            case 'PUSHER_CONNECTED':
                ReactGA.event({ category: 'Pusher', action: 'Connected', label: action.username })
                request(store, 'get_config')
                    .then(
                        response => {
                            if (response.error){
                                console.error(response.error)
                                return false
                            }

                            response.type = 'PUSHER_CONFIG'
                            store.dispatch(response)
                            if (response.config.spotify_username && store.getState().spotify.enabled){
                                store.dispatch(spotifyActions.getUser('spotify:user:'+response.config.spotify_username))
                            }
                            var core = store.getState().core
                            if (!core.country || !core.locale){
                                store.dispatch(coreActions.set({
                                    country: response.config.country,
                                    locale: response.config.locale
                                }))
                            }
                        }
                    )
                request(store, 'get_version')
                    .then(
                        response => {
                            if (response.error){
                                console.error(response.error)
                                return false
                            }
                            response.type = 'PUSHER_VERSION'
                            store.dispatch(response)
                        }
                    )
                request(store, 'get_radio')
                    .then(
                        response => {
                            if (response.error){
                                console.error(response.error)
                                return false
                            }

                            response.type = 'PUSHER_RADIO'
                            store.dispatch(response)
                        }
                    )

                store.dispatch(pusherActions.getQueueMetadata())

                return next(action);
                break;

            case 'PUSHER_INSTRUCT':
                request( action )
                    .then(
                        response => {
                            store.dispatch({ type: 'PUSHER_INSTRUCT', data: response.data })
                        }
                    )
                break

            case 'PUSHER_DELIVER_MESSAGE':
                request(store, 'deliver_message', action.data)
                    .then(
                        response => {
                            store.dispatch( uiActions.createNotification('Message delivered') )
                        }
                    )
                break

            case 'PUSHER_DELIVER_BROADCAST':
                request(store, 'broadcast', action.data)
                break

            case 'PUSHER_GET_QUEUE_METADATA':
                request(store, 'get_queue_metadata')
                    .then(
                        response => {
                            response.type = 'PUSHER_QUEUE_METADATA'
                            store.dispatch(response)
                        }
                    )
                break;

            case 'PUSHER_ADD_QUEUE_METADATA':
                request(store, 'add_queue_metadata', {
                    tlids: action.tlids, 
                    added_from: action.from_uri,
                    added_by: store.getState().pusher.username
                })
                break;

            case 'PUSHER_START_UPGRADE':
                ReactGA.event({ category: 'Pusher', action: 'Upgrade', label: '' })
                request(store, 'upgrade')
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

                        response.type = 'PUSHER_VERSION'
                        store.dispatch(response)
                    }
                )
                return next(action);
                break;

            case 'PUSHER_SET_USERNAME':
                request(store, 'set_username', {
                    username: action.username
                })
                .then(
                    response => {
                        if (response.error){
                            console.error(response.error)
                            return false
                        }
                        response.type = 'PUSHER_USERNAME_CHANGED'
                        store.dispatch(response)
                    }
                )
                return next(action);
                break;

            case 'PUSHER_GET_CONNECTIONS':
                request(store, 'get_connections')
                .then(
                    response => {             
                        if (response.error){
                            console.error(response.error)
                            return false
                        }
                        response.type = 'PUSHER_CONNECTIONS'
                        store.dispatch(response)
                    }
                )
                return next(action);
                break

            case 'PUSHER_SPOTIFY_AUTHORIZATION':
                store.dispatch(uiActions.openModal('receive_authorization', {authorization: action.authorization, user: action.me}))
                break

            case 'PUSHER_START_RADIO':
            case 'PUSHER_UPDATE_RADIO':
                ReactGA.event({ category: 'Pusher', action: 'Start radio', label: action.uris.join() })

                // start our UI process notification  
                if (action.type == 'PUSHER_UPDATE_RADIO'){
                    store.dispatch(uiActions.startProcess('PUSHER_RADIO_PROCESS', 'Updating radio'))
                } else {
                    store.dispatch(uiActions.startProcess('PUSHER_RADIO_PROCESS', 'Starting radio'))
                }

                var data = {
                    update: (action.type == 'PUSHER_UPDATE_RADIO'),
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

                request(store, 'change_radio', data)
                .then(response => {
                    if (response.status == 0){
                        store.dispatch(uiActions.createNotification(response.message, 'bad'))
                    }
                    store.dispatch(uiActions.processFinished('PUSHER_RADIO_PROCESS'))
                })
                break

            case 'PUSHER_STOP_RADIO':
                store.dispatch(uiActions.createNotification('Stopping radio'))
                ReactGA.event({ category: 'Pusher', action: 'Stop radio' })

                var data = {
                    seed_artists: [],
                    seed_genres: [],
                    seed_tracks: []
                }

                // we don't need to wait for response, as change will be broadcast
                request(store, 'stop_radio', data)
                break

            case 'PUSHER_RADIO_STARTED':
            case 'PUSHER_RADIO_CHANGED':
                if (action.radio && action.radio.enabled && store.getState().spotify.enabled){
                    store.dispatch(spotifyActions.resolveRadioSeeds(action.radio))
                }
                next(action)
                break

            case 'PUSHER_BROWSER_NOTIFICATION':
                store.dispatch(uiActions.createBrowserNotification(action))
                break

            case 'PUSHER_RESTART':
                // Hard reload. This doesn't strictly clear the cache, but our compiler's
                // cache buster should handle that 
                window.location.reload(true);
                break

            case 'PUSHER_VERSION':
                ReactGA.event({ category: 'Pusher', action: 'Version', label: action.version.current })

                if (action.version.upgrade_available){
                    store.dispatch( uiActions.createNotification( 'Version '+action.version.latest+' is available. See settings to upgrade.' ) )
                }
                next( action )
                break

            case 'PUSHER_CONFIG':
                store.dispatch(spotifyActions.set({
                    locale: (action.config.locale ? action.config.locale : null),
                    country: (action.config.country ? action.config.country : null),
                    authorization_url: (action.config.authorization_url ? action.config.authorization_url : null),
                    backend_username: (action.config.spotify_username ? action.config.spotify_username : null)
                }))

                // Get our backend_username user
                if (store.getState().spotify.access !== 'none' && (!store.getState().core.users || !store.getState().core.users[action.config.spotify_username])){
                    store.dispatch(spotifyActions.getUser(action.config.spotify_username))
                }

                next( action )
                break

            case 'PUSHER_DEBUG':
                request(store, action.message.method, action.message.data )
                .then(
                    response => {
                        store.dispatch({type: 'DEBUG', response: response})
                    }
                )
                break;

            case 'PUSHER_ERROR':
                store.dispatch(uiActions.createNotification(action.message, 'bad'))
                ReactGA.event({ category: 'Pusher', action: 'Error', label: action.message })
                break

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default PusherMiddleware

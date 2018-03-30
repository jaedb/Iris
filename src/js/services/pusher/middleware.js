
import ReactGA from 'react-ga'

var helpers = require('../../helpers.js')
var coreActions = require('../core/actions.js')
var uiActions = require('../ui/actions.js')
var pusherActions = require('./actions.js')
var lastfmActions = require('../lastfm/actions.js')
var spotifyActions = require('../spotify/actions.js')

const PusherMiddleware = (function(){ 

    // container for the actual websocket
    var socket = null

    // requests pending
    var deferredRequests = []

    // handle all manner of socket messages
    const handleMessage = (ws, store, message) => {

        if (store.getState().ui.log_pusher){
            console.log('Pusher log (incoming)', message);
        }

        // Response with request_id
        if (message.id !== undefined && message.id){

            // Response matches a pending request
            if (deferredRequests[message.id] !== undefined){

                store.dispatch(uiActions.stopLoading(message.id));

                // Response is an error
                if (message.error !== undefined){
                    deferredRequests[message.id].reject(message.error);

                // Successful response
                } else {
                    deferredRequests[message.id].resolve(message.result);
                }

            // Hmm, the response doesn't appear to be for us?
            } else {
                store.dispatch(coreActions.handleException(
                    'Pusher: Response received with no matching request', 
                    message
                ));
            }

        // General broadcast received
        } else {

            // Broadcast of an error
            if (message.error !== undefined){
                store.dispatch(coreActions.handleException(
                    'Pusher: '+message.error.message, 
                    message
                ));
            } else {

                switch (message.method){
                    case 'connection_added':
                        store.dispatch(pusherActions.connectionAdded(message.params.connection));
                        break;
                    case 'connection_changed':
                        store.dispatch(pusherActions.connectionChanged(message.params.connection));
                        break;
                    case 'connection_removed':
                        store.dispatch(pusherActions.connectionRemoved(message.params.connection));
                        break;
                    case 'queue_metadata_changed':
                        store.dispatch(pusherActions.queueMetadataChanged(message.params.queue_metadata));
                        break;
                    case 'spotify_token_changed':
                        store.dispatch(spotifyActions.tokenChanged(message.params.spotify_token));
                        break;
                    case 'spotify_authorization_received':
                        store.dispatch(uiActions.openModal('receive_authorization', message.params));
                        break;
                    case 'notification':
                        store.dispatch(uiActions.createNotification(message.params.notification));
                        break;
                }
            }
        }
    }

    const request = (store, method, params = null) => {
        return new Promise((resolve, reject) => {

            if (store.getState().ui.log_pusher){
                console.log('Pusher log (outgoing)', {method: method, params: params});
            }

            var id = helpers.generateGuid();
            var message = {
                jsonrpc: '2.0',
                id: id,
                method: method
            }
            if (params){
                message.params = params;
            }
            socket.send(JSON.stringify(message));

            store.dispatch(uiActions.startLoading(id, 'pusher_'+method));

            // Start our 15 second timeout
            var timeout = setTimeout(
                function(){
                    store.dispatch(uiActions.stopLoading(id));
                    reject({
                        id: id, 
                        code: 32300, 
                        message: "Request timed out"
                    });
                },
                30000
            );
            
            // add query to our deferred responses
            deferredRequests[id] = {
                resolve: resolve,
                reject: reject
            };
        })
    }

    return store => next => action => {
        switch(action.type){

            case 'PUSHER_CONNECT':

                // Stagnant socket, close it first
                if (socket != null){
                    socket.close();
                }

                store.dispatch({type: 'PUSHER_CONNECTING'});

                var state = store.getState();
                var connection = {
                    client_id: helpers.generateGuid(),
                    connection_id: helpers.generateGuid(),
                    username: 'Anonymous'
                }
                if (state.pusher.username){
                    connection.username = state.pusher.username;
                }
                connection.username = connection.username.replace(/\W/g, '');
                
                socket = new WebSocket(
                    'ws'+(window.location.protocol === 'https:' ? 's' : '')+'://'+state.mopidy.host+':'+state.mopidy.port+'/iris/ws/',
                    [ connection.client_id, connection.connection_id, connection.username ]
                );

                socket.onopen = () => {
                    store.dispatch({
                        type: 'PUSHER_CONNECTED',
                        connection_id: connection.connection_id,
                        client_id: connection.client_id,
                        username: connection.username
                    });
                };

                socket.onclose = () => {
                    store.dispatch({
                        type: 'PUSHER_DISCONNECTED'
                    })

                    // attempt to reconnect every 5 seconds
                    setTimeout(() => {
                        store.dispatch(pusherActions.connect())
                    }, 5000);
                };

                socket.onmessage = (message) => {
                    var message = JSON.parse(message.data);
                    handleMessage(socket, store, message);
                };

                break;

            case 'PUSHER_CONNECTED':
                ReactGA.event({ category: 'Pusher', action: 'Connected', label: action.username});

                store.dispatch(pusherActions.getConfig());
                store.dispatch(pusherActions.getVersion());
                store.dispatch(pusherActions.getRadio());
                store.dispatch(pusherActions.getQueueMetadata());
                
                next(action);
                break;

            case 'PUSHER_INSTRUCT':
                request(action)
                    .then(
                        response => {
                            store.dispatch({ type: 'PUSHER_INSTRUCT', data: response.data })
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Instruct failed',
                                error
                            ));
                        }
                    );
                break

            case 'PUSHER_DELIVER_MESSAGE':
                request(store, 'send_message', action.data)
                    .then(
                        response => {
                            store.dispatch(uiActions.createNotification({content: 'Message delivered'}));
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Could not deliver message',
                                error
                            ));
                        }
                    );
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
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Could not load queue metadata',
                                error
                            ));
                        }
                    );
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
                            if (response.upgrade_successful){
                                store.dispatch(uiActions.createNotification({content: 'Upgrade complete'}));
                            } else {
                                store.dispatch(uiActions.createNotification({content: 'Upgrade failed, please upgrade manually', type: 'bad'}));
                            }

                            response.type = 'PUSHER_VERSION'
                            store.dispatch(response)
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Could not start upgrade',
                                error
                            ));
                        }
                    );
                return next(action);
                break;

            case 'PUSHER_SET_USERNAME':
                request(store, 'set_username', {username: action.username})
                    .then(
                        response => {
                            response.type = 'PUSHER_USERNAME_CHANGED'
                            store.dispatch(response)
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Could not set username',
                                error
                            ));
                        }
                    );
                return next(action);
                break;

            case 'PUSHER_GET_VERSION':
                request(store, 'get_version')
                    .then(
                        response => {
                            store.dispatch({
                                type: 'PUSHER_VERSION',
                                version: response.version
                            })
                        },
                        error => {                        
                            store.dispatch(coreActions.handleException(
                                'Could not load version',
                                error
                            ));
                        }
                    );
                break;

            case 'PUSHER_GET_CONFIG':
                request(store, 'get_config')
                    .then(
                        response => {
                            store.dispatch({
                                type: 'PUSHER_CONFIG',
                                config: response.config
                            });

                            var core = store.getState().core;
                            if (!core.country || !core.locale){
                                store.dispatch(spotifyActions.set({
                                    country: response.config.country,
                                    locale: response.config.locale
                                }))
                            }
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Could not load config',
                                error
                            ));
                        }
                    );
                break;

            case 'PUSHER_GET_CONNECTIONS':
                request(store, 'get_connections')
                    .then(
                        response => {
                            store.dispatch({
                                type: 'PUSHER_CONNECTIONS',
                                connections: response.connections
                            })
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Could not load connections',
                                error
                            ));
                        }
                    );
                return next(action);
                break;

            case 'PUSHER_GET_RADIO':
                request(store, 'get_radio')
                    .then(
                        response => {
                            store.dispatch({
                                type: 'PUSHER_RADIO',
                                radio: response.radio
                            });
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Could not load radio',
                                error
                            ));
                        }
                    );
                break;

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
                
                for(var i = 0; i < action.uris.length; i++){
                    switch(helpers.uriType(action.uris[i] )){
                        case 'artist':
                            data.seed_artists.push(action.uris[i] );
                            break;
                        case 'track':
                            data.seed_tracks.push(action.uris[i] );
                            break;
                        case 'genre':
                            data.seed_genres.push(action.uris[i] );
                            break;
                    }
                }

                request(store, 'change_radio', data)
                    .then(
                        response => {
                            store.dispatch(uiActions.processFinishing('PUSHER_RADIO_PROCESS'));
                            if (response.status == 0){
                                store.dispatch(uiActions.createNotification({content: response.message, type: 'bad'}));
                            }
                        },
                        error => {       
                            store.dispatch(uiActions.processFinishing('PUSHER_RADIO_PROCESS'));                     
                            store.dispatch(coreActions.handleException(
                                'Could not change radio',
                                error
                            ));
                        }
                    )
                break

            case 'PUSHER_STOP_RADIO':
                store.dispatch(uiActions.createNotification({content: 'Stopping radio'}));
                ReactGA.event({ category: 'Pusher', action: 'Stop radio' });

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

            case 'PUSHER_NOTIFICATION':
                var data = Object.assign(
                    {}, 
                    action, {
                        type: action.notification_type
                    }
                );
                store.dispatch(uiActions.createNotification(data));
                break

            case 'PUSHER_RESTART':
                // Hard reload. This doesn't strictly clear the cache, but our compiler's
                // cache buster should handle that 
                window.location.reload(true);
                break

            case 'PUSHER_VERSION':
                ReactGA.event({ category: 'Pusher', action: 'Version', label: action.version.current })

                if (action.version.upgrade_available){
                    store.dispatch(uiActions.createNotification({content: 'Version '+action.version.latest+' is available. See settings to upgrade.'}));
                }
                next(action )
                break

            case 'PUSHER_CONFIG':
                store.dispatch(spotifyActions.set({
                    locale: (action.config.locale ? action.config.locale : null),
                    country: (action.config.country ? action.config.country : null),
                    authorization_url: (action.config.spotify_authorization_url ? action.config.spotify_authorization_url : null)
                }));
                store.dispatch(lastfmActions.set({
                    authorization_url: (action.config.lastfm_authorization_url ? action.config.lastfm_authorization_url : null)
                }));

                next(action);
                break

            case 'PUSHER_DEBUG':
                request(store, action.message.method, action.message.data )
                    .then(
                        response => {
                            store.dispatch({type: 'DEBUG', response: response})
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Could not debug',
                                error,
                                error.message
                            ));
                        }
                    );
                break;

            case 'PUSHER_ERROR':
                store.dispatch(uiActions.createNotification(action.message, 'bad'))
                ReactGA.event({ category: 'Pusher', action: 'Error', label: action.message })
                break


            /**
             * Snapcast actions
             **/
            case 'PUSHER_GET_SNAPCAST':
                request(store, 'snapcast_instruct', action.data)
                    .then(
                        response => {
                            var groups = {};
                            var clients = {};

                            // Loop all the groups
                            for (var i = 0; i < response.server.groups.length; i++){
                                var group = response.server.groups[i];
                                var clients_ids = [];

                                // And now this groups' clients
                                for (var j = 0; j < group.clients.length; j++){
                                    var client = group.clients[j];
                                    clients[client.id] = client;
                                    clients_ids.push(client.id);
                                }

                                groups[group.id] = {
                                    id: group.id,
                                    muted: group.muted,
                                    name: group.name,
                                    stream_id: group.stream_id,
                                    clients_ids: clients_ids
                                }
                            }

                            store.dispatch({
                                type: 'PUSHER_SNAPCAST', 
                                snapcast_clients: clients,
                                snapcast_groups: groups
                            });
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Could not get Snapcast server',
                                error,
                                error.message
                            ));
                        }
                    );
                break

            case 'PUSHER_SET_SNAPCAST_CLIENT_VOLUME':
                request(store, 'snapcast_instruct', action.data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'PUSHER_SNAPCAST_CLIENT_UPDATED', 
                                key: action.data.params.id,
                                client: {
                                    config: {
                                        volume: response.volume
                                    }
                                }
                            })
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Error',
                                error,
                                error.message
                            ));
                        }
                    );
                break

            case 'PUSHER_SET_SNAPCAST_CLIENT_NAME':
                request(store, 'snapcast_instruct', action.data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'PUSHER_SNAPCAST_CLIENT_UPDATED', 
                                key: action.data.params.id,
                                client: {
                                    config: {
                                        name: response.name
                                    }
                                }
                            })
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Error',
                                error,
                                error.message
                            ));
                        }
                    );
                break

            case 'PUSHER_SET_SNAPCAST_CLIENT_LATENCY':
                request(store, 'snapcast_instruct', action.data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'PUSHER_SNAPCAST_CLIENT_UPDATED', 
                                key: action.data.params.id,
                                client: {
                                    config: {
                                        latency: response.latency
                                    }
                                }
                            })
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Error',
                                error,
                                error.message
                            ));
                        }
                    );
                break

            case 'PUSHER_DELETE_SNAPCAST_CLIENT':
                request(store, 'snapcast_instruct', action.data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'PUSHER_SNAPCAST_CLIENT_REMOVED', 
                                key: action.data.params.id
                            })
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Error',
                                error,
                                error.message
                            ));
                        }
                    );
                break

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default PusherMiddleware


import ReactGA from 'react-ga'

var helpers = require('../../helpers.js')
var coreActions = require('../core/actions.js')
var uiActions = require('../ui/actions.js')
var mopidyActions = require('../mopidy/actions.js')
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

        // Pull our ID. JSON-RPC nests the ID under the error object, 
        // so make sure we handle that.
        // TODO: Use this as our measure of a successful response vs error
        var id = null;
        if (message.id){
            id = message.id;
        } else if (message.error && message.error.id){
            id = message.error.id;
        }

        // Response with request_id
        if (id){

            // Response matches a pending request
            if (deferredRequests[id] !== undefined){

                store.dispatch(uiActions.stopLoading(id));

                // Response is an error
                if (message.error !== undefined){
                    deferredRequests[id].reject(message.error);

                // Successful response
                } else {
                    deferredRequests[id].resolve(message.result);
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
                    message,
                    (message.error.data !== undefined && message.error.data.description !== undefined ? message.error.data.description : null)
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
                    case 'radio_started':
                        store.dispatch(pusherActions.radioStarted(message.params.radio));
                        break;
                    case 'radio_changed':
                        store.dispatch(pusherActions.radioChanged(message.params.radio));
                        break;
                    case 'radio_stopped':
                        store.dispatch(pusherActions.radioStopped());
                        break;
                    case 'reload':
                        window.location.reload(true);
                        break;
                    case 'upgrading':
                        store.dispatch(uiActions.createNotification({content: 'Upgrading...', type: 'info'}));
                        break;
                    case 'restarting':
                        store.dispatch(uiActions.createNotification({content: 'Restarting...', type: 'info'}));
                        break;
                }
            }
        }
    }

    const request = (store, method, params = null) => {
        return new Promise((resolve, reject) => {

            var id = helpers.generateGuid();
            var message = {
                jsonrpc: '2.0',
                id: id,
                method: method
            }
            if (params){
                message.params = params;
            }

            if (store.getState().ui.log_pusher){
                console.log('Pusher log (outgoing)', message);
            }

            socket.send(JSON.stringify(message));

            store.dispatch(uiActions.startLoading(id, 'pusher_'+method));

            // Start our 30 second timeout
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
                if (store.getState().ui.allow_reporting){
	                ReactGA.event({ category: 'Pusher', action: 'Connected', label: action.username});
	            }

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
                                type: 'PUSHER_RADIO_LOADED',
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
                if (store.getState().ui.allow_reporting){
	                ReactGA.event({ category: 'Pusher', action: 'Start radio', label: action.uris.join() });
	            }

                // start our UI process notification  
                if (action.type == 'PUSHER_UPDATE_RADIO'){
                    store.dispatch(uiActions.startProcess('PUSHER_RADIO_PROCESS', 'Updating radio'))
                } else {
                    store.dispatch(uiActions.startProcess('PUSHER_RADIO_PROCESS', 'Starting radio'))
                }

                var data = {
                    reset: (action.type == 'PUSHER_START_RADIO'),
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

                if (action.type == 'PUSHER_START_RADIO'){
                    store.dispatch(pusherActions.deliverBroadcast(
                        'notification',
                        {
                            notification: {
                                type: 'info',
                                content: store.getState().pusher.username + ' is starting radio mode'
                            }
                        }
                    ));
                }

                request(store, 'change_radio', data)
                    .then(
                        response => {
                            store.dispatch(uiActions.processFinishing('PUSHER_RADIO_PROCESS'));
                            if (response.status == 0){
                                store.dispatch(uiActions.createNotification({content: response.message, type: 'bad'}));
                            }
                            store.dispatch(pusherActions.radioChanged(response.radio));
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

                if (store.getState().ui.allow_reporting){
	                ReactGA.event({ category: 'Pusher', action: 'Stop radio' });
	            }

                store.dispatch(pusherActions.deliverBroadcast(
                    'notification',
                    {
                        notification: {
                            type: 'info',
                            content: store.getState().pusher.username + ' stopped radio mode'
                        }
                    }
                ));

                var data = {
                    seed_artists: [],
                    seed_genres: [],
                    seed_tracks: []
                }

                request(store, 'stop_radio', data)
                    .then(
                        response => {
                            store.dispatch(pusherActions.radioStopped());
                        }, error => {                 
                            store.dispatch(coreActions.handleException(
                                'Could not stop radio',
                                error
                            ));
                        }
                    );
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

            case 'PUSHER_RELOAD':
                // Hard reload. This doesn't strictly clear the cache, but our compiler's
                // cache buster should handle that 
                window.location.reload(true);
                break

            case 'PUSHER_RESTART':
                request(store, 'restart')
                    .then(
                        response => {
                            store.dispatch(mopidyActions.restarting());
                        },
                        error => {
                            store.dispatch(uiActions.createNotification({content: error.message, description: (error.description ? error.description : null), type: 'bad'}));
                        }
                    );
                next(action);
                break

            case 'PUSHER_UPGRADE':
                if (store.getState().ui.allow_reporting){
	                ReactGA.event({ category: 'Pusher', action: 'Upgrade', label: '' });
	            }
                request(store, 'upgrade')
                    .then(
                        response => {
                            store.dispatch(mopidyActions.upgrading());
                        },
                        error => {
                            store.dispatch(uiActions.createNotification({content: error.message, description: (error.description ? error.description : null), type: 'bad'}));
                        }
                    );
                break;

            case 'PUSHER_VERSION':
                if (store.getState().ui.allow_reporting){
	                ReactGA.event({ category: 'Pusher', action: 'Version', label: action.version.current });
	            }

                if (action.version.upgrade_available){
                    store.dispatch(uiActions.createNotification({content: 'Version '+action.version.latest+' is available. See settings to upgrade.'}));
                }
                next(action);
                break

            case 'PUSHER_CONFIG':

                // Set default country/locale (unless we've already been configured)
                var spotify = store.getState().spotify;
                var spotify_updated = false;
                var spotify_updates = {};

                if (!spotify.country && action.config.country){
                    spotify_updates.country = action.config.country;
                    spotify_updated = true;
                }

                if (!spotify.locale && action.config.locale){
                    spotify_updates.locale = action.config.locale;
                    spotify_updated = true;
                }

                if (action.config.spotify_authorization_url){
                    spotify_updates.authorization_url = action.config.spotify_authorization_url;
                    spotify_updated = true;
                }

                if (spotify_updated){
                    store.dispatch(spotifyActions.set(spotify_updates));
                }
                
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
                store.dispatch(uiActions.createNotification(action.message, 'bad'));
                if (store.getState().ui.allow_reporting){
	                ReactGA.event({ category: 'Pusher', action: 'Error', label: action.message });
	            }
                break


            /**
             * Snapcast actions
             **/
            case 'PUSHER_GET_SNAPCAST':
                request(store, 'snapcast_instruct', action.data)
                    .then(
                        response => {
                            store.dispatch(pusherActions.snapcastServerLoaded(response.server));
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

            case 'PUSHER_SNAPCAST_SERVER_LOADED':
                var groups = {};
                var clients = {};
                var streams = {};

                // Loop all the groups
                for (var i = 0; i < action.server.groups.length; i++){
                    var group = action.server.groups[i];
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

                // Loop all the streams
                for (var i = 0; i < action.server.streams.length; i++){
                    var stream = action.server.streams[i];
                    streams[stream.id] = stream;
                }

                store.dispatch({
                    type: 'PUSHER_SNAPCAST', 
                    snapcast_clients: clients,
                    snapcast_groups: groups,
                    snapcast_streams: streams
                });
                break

            case 'PUSHER_SET_SNAPCAST_CLIENT_NAME':
                var client = store.getState().pusher.snapcast_clients[action.id];
                var data = {
                    method: 'Client.SetName',
                    params: {
                        id: action.id,
                        name: action.name
                    }
                }

                request(store, 'snapcast_instruct', data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'PUSHER_SNAPCAST_CLIENT_UPDATED', 
                                key: action.id,
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

            case 'PUSHER_SET_SNAPCAST_CLIENT_MUTE':
                var client = store.getState().pusher.snapcast_clients[action.id];
                var data = {
                    method: 'Client.SetVolume',
                    params: {
                        id: action.id,
                        volume: {
                            muted: action.mute,
                            percent: client.config.volume.percent,
                        }
                    }
                }

                request(store, 'snapcast_instruct', data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'PUSHER_SNAPCAST_CLIENT_UPDATED', 
                                key: action.id,
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

            case 'PUSHER_SET_SNAPCAST_CLIENT_VOLUME':
                var client = store.getState().pusher.snapcast_clients[action.id];
                var data = {
                    method: 'Client.SetVolume',
                    params: {
                        id: action.id,
                        volume: {
                            muted: client.config.volume.muted,
                            percent: action.percent
                        }
                    }
                }

                request(store, 'snapcast_instruct', data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'PUSHER_SNAPCAST_CLIENT_UPDATED', 
                                key: action.id,
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

            case 'PUSHER_SET_SNAPCAST_CLIENT_LATENCY':
                var client = store.getState().pusher.snapcast_clients[action.id];
                var data = {
                    method: 'Client.SetLatency',
                    params: {
                        id: action.id,
                        latency: action.latency
                    }
                }

                request(store, 'snapcast_instruct', data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'PUSHER_SNAPCAST_CLIENT_UPDATED', 
                                key: action.id,
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

            case 'PUSHER_SET_SNAPCAST_CLIENT_GROUP':

                var group = store.getState().pusher.snapcast_groups[action.group_id];
                var clients_ids = group.clients_ids;
                var clients_ids_index = clients_ids.indexOf(action.id);

                // Not in group (yet), so add it
                if (clients_ids_index <= -1){
                    clients_ids.push(action.id);

                // Already there, so remove it
                } else {
                    clients_ids.splice(clients_ids_index, 1);
                }

                var data = {
                    method: 'Group.SetClients',
                    params: {
                        id: action.group_id,
                        clients: clients_ids
                    }
                }

                request(store, 'snapcast_instruct', data)
                    .then(
                        response => {
                            store.dispatch(pusherActions.snapcastServerLoaded(response.server));
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
                var data = {
                    method: 'Server.DeleteClient',
                    params: {
                        id: action.id
                    }
                }

                request(store, 'snapcast_instruct', data)
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

            case 'PUSHER_SET_SNAPCAST_GROUP_STREAM':
                var group = store.getState().pusher.snapcast_groups[action.id];
                var data = {
                    method: 'Group.SetStream',
                    params: {
                        id: action.id,
                        stream_id: action.stream_id
                    }
                }

                request(store, 'snapcast_instruct', data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'PUSHER_SNAPCAST_GROUP_UPDATED', 
                                key: action.id,
                                group: {
                                    stream_id: action.stream_id
                                }
                            })
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Could not change stream',
                                error,
                                error.message
                            ));
                        }
                    );
                break

            case 'PUSHER_SET_SNAPCAST_GROUP_MUTE':
                var group = store.getState().pusher.snapcast_groups[action.id];
                var data = {
                    method: 'Group.SetMute',
                    params: {
                        id: action.id,
                        mute: action.mute
                    }
                }

                request(store, 'snapcast_instruct', data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'PUSHER_SNAPCAST_GROUP_UPDATED', 
                                key: action.id,
                                group: {
                                    muted: response.mute
                                }
                            })
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Could not toggle mute',
                                error,
                                error.message
                            ));
                        }
                    );
                break

            case 'PUSHER_SET_SNAPCAST_GROUP_VOLUME':
                var clients_to_update = [];
                var group = store.getState().pusher.snapcast_groups[action.id];
                var change = action.percent - action.old_percent;

                for (var i = 0; i < group.clients_ids.length; i++){

                    // Apply the change proportionately to each client
                    var client = store.getState().pusher.snapcast_clients[group.clients_ids[i]];
                    var current_percent = client.config.volume.percent;
                    var new_percent = current_percent + change;

                    // Only change if the client is within min/max limits
                    if ((change > 0 && current_percent < 100) || (change < 0 && current_percent > 0)){
                        clients_to_update.push({
                            id: client.id,
                            percent: new_percent
                        });
                    }
                }

                // Loop our required changes, and post each to Snapcast
                for (var i = 0; i < clients_to_update.length; i++){
                    var update = clients_to_update[i];
                    var percent = update.percent + ((group.clients_ids.length - clients_to_update.length) * change);

                    // Make sure we're not creating an impossible percent
                    if (percent < 0){
                        percent = 0;
                    } else if (percent > 100){
                        percent = 100;
                    }

                    store.dispatch(pusherActions.setSnapcastClientVolume(update.id, percent));
                }

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default PusherMiddleware

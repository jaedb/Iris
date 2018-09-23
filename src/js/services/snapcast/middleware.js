
import ReactGA from 'react-ga'

var helpers = require('../../helpers')
var coreActions = require('../core/actions')
var uiActions = require('../ui/actions')
var pusherActions = require('../pusher/actions')

const SnapcastMiddleware = (function(){ 

    const request = (store, params = null) => {
        store.dispatch(pusherActions.instruct('snapcast_instruct', params));
        return;
    }

    return store => next => action => {
        switch(action.type){

            case 'SNAPCAST_GET_SERVER':
                console.log( request(store, {method: 'Server.GetStatus'}) )/*
                    .then(
                        response => {
                            store.dispatch(snapcastActions.serverLoaded(response.server));
                        },
                        error => {                            
                            store.dispatch(coreActions.handleException(
                                'Could not get Snapcast server',
                                error,
                                error.message
                            ));
                        }
                    );*/
                break

            case 'SNAPCAST_SERVER_LOADED':
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

                action.clients = clients;
                action.groups = groups;
                action.streams = streams;
                next(action);
                break

            case 'SNAPCAST_SET_CLIENT_NAME':
                var client = store.getState().snapcast.clients[action.id];
                var data = {
                    method: 'Client.SetName',
                    params: {
                        id: action.id,
                        name: action.name
                    }
                }

                request(store, data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'SNAPCAST_CLIENT_UPDATED', 
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

            case 'SNAPCAST_SET_CLIENT_MUTE':
                var client = store.getState().snapcast.clients[action.id];
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

                request(store, data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'SNAPCAST_CLIENT_UPDATED', 
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

            case 'SNAPCAST_SET_CLIENT_VOLUME':
                var client = store.getState().snapcast.clients[action.id];
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

                request(store, data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'SNAPCAST_CLIENT_UPDATED', 
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

            case 'SNAPCAST_SET_CLIENT_LATENCY':
                var client = store.getState().snapcast.clients[action.id];
                var data = {
                    method: 'Client.SetLatency',
                    params: {
                        id: action.id,
                        latency: action.latency
                    }
                }

                request(store, data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'SNAPCAST_CLIENT_UPDATED', 
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

            case 'SNAPCAST_SET_CLIENT_GROUP':

                var group = store.getState().snapcast.groups[action.group_id];
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

                request(store, data)
                    .then(
                        response => {
                            store.dispatch(snapcastActions.serverLoaded(response.server));
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

            case 'SNAPCAST_DELETE_CLIENT':
                var data = {
                    method: 'Server.DeleteClient',
                    params: {
                        id: action.id
                    }
                }

                request(store, data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'SNAPCAST_CLIENT_REMOVED', 
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

            case 'SNAPCAST_SET_GROUP_STREAM':
                var group = store.getState().snapcast.groups[action.id];
                var data = {
                    method: 'Group.SetStream',
                    params: {
                        id: action.id,
                        stream_id: action.stream_id
                    }
                }

                request(store, data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'SNAPCAST_GROUP_UPDATED', 
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

            case 'SNAPCAST_SET_GROUP_MUTE':
                var group = store.getState().snapcast.groups[action.id];
                var data = {
                    method: 'Group.SetMute',
                    params: {
                        id: action.id,
                        mute: action.mute
                    }
                }

                request(store, data)
                    .then(
                        response => {
                            store.dispatch({
                                type: 'SNAPCAST_GROUP_UPDATED', 
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

            case 'SNAPCAST_SET_GROUP_VOLUME':
                var clients_to_update = [];
                var group = store.getState().snapcast.groups[action.id];
                var change = action.percent - action.old_percent;

                for (var i = 0; i < group.clients_ids.length; i++){

                    // Apply the change proportionately to each client
                    var client = store.getState().snapcast.clients[group.clients_ids[i]];
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

                    store.dispatch(snapcastActions.setClientVolume(update.id, percent));
                }

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default SnapcastMiddleware

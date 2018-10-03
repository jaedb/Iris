
export default function reducer(snapcast = {}, action){
    switch (action.type){

        case 'SNAPCAST_SERVER_LOADED':
            var server = Object.assign({}, action.server);
            return Object.assign({}, snapcast, { server: server });

        case 'SNAPCAST_CLIENT_COMMANDS_UPDATED':
            return Object.assign({}, snapcast, { client_commands: action.client_commands });

        case 'SNAPCAST_CLIENTS_LOADED':
            if (action.flush){
                var clients = {};
            } else {
                var clients = Object.assign({}, snapcast.clients);
            }

            for (var client of action.clients){
                clients[client.id] = client;
            }
            return Object.assign({}, snapcast, { clients: clients });

        case 'SNAPCAST_GROUPS_LOADED':
            if (action.flush){
                var groups = {};
            } else {
                var groups = Object.assign({}, snapcast.groups);
            }

            for (var group of action.groups){
                groups[group.id] = group;
            }
            return Object.assign({}, snapcast, { groups: groups });

        case 'SNAPCAST_STREAMS_LOADED':
            if (action.flush){
                var streams = {};
            } else {
                var streams = Object.assign({}, snapcast.streams);
            }

            for (var stream of action.streams){
                streams[stream.id] = stream;
            }
            return Object.assign({}, snapcast, { streams: streams });

        default:
            return snapcast
    }
}




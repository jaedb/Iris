
export default function reducer(snapcast = {}, action){
    switch (action.type){

        case 'SNAPCAST_SERVER_LOADED':
            return Object.assign({}, snapcast, { 
                streams: action.streams,
                groups: action.groups,
                clients: action.clients
            });

        case 'SNAPCAST_GROUP_UPDATED':
            var groups = Object.assign({}, snapcast.groups);
            var group = groups[action.key];
            groups[action.key] = Object.assign({}, group, action.group);
            return Object.assign({}, snapcast, { groups: groups });

        case 'SNAPCAST_CLIENT_UPDATED':
            var clients = Object.assign({}, snapcast.clients);
            var client = clients[action.key];
            client.config = Object.assign({}, client.config, action.client.config);
            clients[action.key] = client;
            return Object.assign({}, snapcast, { clients: clients });

        case 'SNAPCAST_CLIENT_REMOVED':
            var clients = Object.assign({}, snapcast.clients);
            delete clients[action.key];
            return Object.assign({}, snapcast, { clients: clients });

        default:
            return snapcast
    }
}




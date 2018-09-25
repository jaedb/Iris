
export default function reducer(snapcast = {}, action){
    switch (action.type){

        case 'SNAPCAST_CLIENTS_LOADED':
            var clients = Object.assign({}, snapcast.clients);
            for (var client of action.clients){
                clients[client.uri] = client;
            }
            return Object.assign({}, snapcast, { clients: clients });

        case 'SNAPCAST_SOURCES_LOADED':
            var sources = Object.assign({}, snapcast.sources);
            for (var source of action.sources){
                sources[source.uri] = source;
            }
            return Object.assign({}, snapcast, { sources: sources });

        case 'SNAPCAST_GROUPS_LOADED':
            var groups = Object.assign({}, snapcast.groups);
            for (var group of action.groups){
                groups[group.uri] = group;
            }
            return Object.assign({}, snapcast, { groups: groups });

        default:
            return snapcast
    }
}





export function getServer(){
	return {
		type: 'SNAPCAST_GET_SERVER'
	}
}

export function setClientName(id, name){
	return {
		type: 'SNAPCAST_SET_CLIENT_NAME',
		id: id,
		name: name
	}
}

export function setClientMute(id, mute){
	return {
		type: 'SNAPCAST_SET_CLIENT_MUTE',
		id: id,
		mute: mute
	}
}

export function setClientVolume(id, volume){
	return {
		type: 'SNAPCAST_SET_CLIENT_VOLUME',
		id: id,
		volume: volume
	}
}

export function setClientLatency(id, latency){
	return {
		type: 'SNAPCAST_SET_CLIENT_LATENCY',
		id: id,
		latency: latency
	}
}

export function setClientCommand(id, name, command){
	return {
		type: 'SNAPCAST_SET_CLIENT_COMMAND',
		id: id,
		name: name,
		command: command
	}
}

export function setClientGroup(id, group_id){
	return {
		type: 'SNAPCAST_SET_CLIENT_GROUP',
		id: id,
		group_id: group_id
	}
}

export function deleteClient(id){
	return {
		type: 'SNAPCAST_DELETE_CLIENT',
		id: id
	}
}

export function setGroupStream(id, stream_id){
	return {
		type: 'SNAPCAST_SET_GROUP_STREAM',
		id: id,
		stream_id: stream_id
	}
}

export function setGroupMute(id, mute){
	return {
		type: 'SNAPCAST_SET_GROUP_MUTE',
		id: id,
		mute: mute
	}
}

export function setGroupVolume(id, percent, old_percent = 0){
	return {
		type: 'SNAPCAST_SET_GROUP_VOLUME',
		id: id,
		percent: percent,
		old_percent: old_percent
	}
}




/**
 * Record loaders
 *
 * We've got a loaded record, now we just need to plug it in to our state and stores.
 **/

export function serverLoaded(server){
    return {
        type: 'SNAPCAST_SERVER_LOADED',
        server: server
    }
}

export function clientLoaded(client){
    return clientsLoaded([client]);
}
export function clientsLoaded(clients){
    return {
        type: 'SNAPCAST_CLIENTS_LOADED',
        clients: clients
    }
}

export function groupLoaded(group){
    return groupsLoaded([group]);
}
export function groupsLoaded(groups){
    return {
        type: 'SNAPCAST_GROUPS_LOADED',
        groups: groups
    }
}

export function streamLoaded(stream){
    return streamsLoaded([stream]);
}
export function streamsLoaded(streams){
    return {
        type: 'SNAPCAST_STREAMS_LOADED',
        streams: streams
    }
}
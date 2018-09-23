
export function connect(){
	return {
		type: 'SNAPCAST_CONNECT'
	}
}

export function disconnect(){
	return {
		type: 'SNAPCAST_DISCONNECT'
	}
}

export function reload(){
	return {
		type: 'SNAPCAST_RELOAD'
	}
}

export function getConnections(){
	return {
		type: 'SNAPCAST_GET_CONNECTIONS'
	}
}

export function connectionAdded(connection){
	return {
		type: 'SNAPCAST_CONNECTION_ADDED',
		connection: connection
	}
}

export function connectionChanged(connection){
	return {
		type: 'SNAPCAST_CONNECTION_CHANGED',
		connection: connection
	}
}

export function connectionRemoved(connection){
	return {
		type: 'SNAPCAST_CONNECTION_REMOVED',
		connection: connection
	}
}

export function getServer(){
	return {
		type: 'SNAPCAST_GET_SERVER'
	}
}

export function serverLoaded(server){
	return {
		type: 'SNAPCAST_SERVER_LOADED',
		server: server
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

export function setClientVolume(id, percent){
	return {
		type: 'SNAPCAST_SET_CLIENT_VOLUME',
		id: id,
		percent: percent
	}
}

export function setClientLatency(id, latency){
	return {
		type: 'SNAPCAST_SET_CLIENT_LATENCY',
		id: id,
		latency: latency
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


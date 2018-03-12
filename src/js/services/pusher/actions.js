
/**
 * Actions and Action Creators
 **/

export function setPort(port){
	return {
		type: 'PUSHER_SET_PORT',
		port: port
	}
}

export function setUsername(username){
	return {
		type: 'PUSHER_SET_USERNAME',
		username: username.replace(/[\W_]+/g,'')
	}
}

export function connect(){
	return {
		type: 'PUSHER_CONNECT'
	}
}

export function disconnect(){
	return {
		type: 'PUSHER_DISCONNECT'
	}
}

export function startUpgrade(){
	return {
		type: 'START_UPGRADE'
	}
}

export function getConnections(){
	return {
		type: 'PUSHER_GET_CONNECTIONS'
	}
}

export function instruct(data = null){
	return {
		type: 'PUSHER_INSTRUCT',
		data: data
	}
}

export function getConfig(){
	return {
		type: 'PUSHER_GET_CONFIG'
	}
}

export function getVersion(){
	return {
		type: 'PUSHER_GET_VERSION'
	}
}

export function deliverBroadcast(data = null){
	return {
		type: 'PUSHER_DELIVER_BROADCAST',
		data: data
	}
}

export function sendAuthorization(recipient_connectionid, authorization, me){
	return {
		type: 'PUSHER_DELIVER_MESSAGE',
		data: {
			connection_id: recipient_connectionid,
			message: {
				type: 'spotify_authorization',
				authorization: authorization,
				me: me
			}
		}
	}
}

export function getRadio(){
	return {
		type: 'PUSHER_GET_RADIO'
	}
}

export function startRadio(uris){
	return {
		type: 'PUSHER_START_RADIO',
		uris: uris
	}
}

export function updateRadio(uris){
	return {
		type: 'PUSHER_UPDATE_RADIO',
		uris: uris
	}
}

export function stopRadio(){
	return {
		type: 'PUSHER_STOP_RADIO'
	}
}

export function debug(message = null){
	return {
		type: 'PUSHER_DEBUG',
		message: message
	}
}

export function getQueueMetadata(){
	return {
		type: 'PUSHER_GET_QUEUE_METADATA'
	}
}

export function addQueueMetadata(tlids = [], from_uri = null){
	return {
		type: 'PUSHER_ADD_QUEUE_METADATA',
		tlids: tlids,
		from_uri: from_uri
	}
}

/**
 * Snapcast actions
 * TODO: Figure out how to cleanly split this out to it's own service
 * but still share the pusher middleware connection
 **/

export function getSnapcast(){
	return {
		type: 'PUSHER_GET_SNAPCAST',
		data: {
			method: 'Server.GetStatus'
		}
	}
}

export function setSnapcastClientVolume(id, muted, percent){
	return {
		type: 'PUSHER_SET_SNAPCAST_CLIENT_VOLUME',
		data: {
			method: 'Client.SetVolume',
			params: {
				id: id,
				volume: {
					muted: muted,
					percent: percent
				}
			}
		}
	}
}


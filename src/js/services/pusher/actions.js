
/**
 * Actions and Action Creators
 **/

export function setPort( port ){
	return {
		type: 'PUSHER_SET_PORT',
		port: port
	}
}

export function setUsername( username ){
	return {
		type: 'PUSHER_SET_USERNAME',
		username: username
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

export function instruct( data = null ){
	return {
		type: 'PUSHER_INSTRUCT',
		data: data
	}
}

export function deliverBroadcast( data = null ){
	return {
		type: 'PUSHER_DELIVER_BROADCAST',
		data: data
	}
}

export function sendAuthorization( recipient_connectionid, authorization, me ){
	return {
		type: 'PUSHER_DELIVER_MESSAGE',
		data: {
			to: recipient_connectionid,
			message: {
				type: 'spotify_authorization',
				authorization: authorization,
				me: me
			}
		}
	}
}

export function startRadio( uris ){
	return {
		type: 'PUSHER_START_RADIO',
		uris: uris
	}
}

export function updateRadio( uris ){
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

export function debug( message = null ){
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

export function addQueueMetadata( tlids = [], from_uri = null ){
	return {
		type: 'PUSHER_ADD_QUEUE_METADATA',
		tlids: tlids,
		from_uri: from_uri
	}
}
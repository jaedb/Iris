
/**
 * Actions and Action Creators
 **/

export function setPort( port ){
	return {
		type: 'PUSHER_SET_PORT',
		port: port
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

export function performUpgrade(){
	return {
		type: 'PUSHER_UPGRADING'
	}
}

export function getConnectionList(){
	return {
		type: 'PUSHER_INSTRUCT',
		action: 'get_connections'
	}
}

export function instruct( message_type, data = null ){
	return {
		type: 'PUSHER_INSTRUCT',
		message_type: message_type,
		data: data
	}
}

export function debug( call, data = null ){
	return {
		type: 'PUSHER_DEBUG',
		call: call,
		data: data
	}
}
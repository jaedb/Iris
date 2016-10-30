
/**
 * Actions and Action Creators
 **/

export function setPort( port ){
	return {
		type: 'PUSHER_SET_PORT',
		port: port
	}
}

export function changeUsername( username ){
	return {
		type: 'PUSHER_INSTRUCT',
		action: 'change_username',
		data: username
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

export function instruct( action, data = null ){
	return {
		type: 'PUSHER_INSTRUCT',
		action: action,
		data: data
	}
}
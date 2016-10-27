
/**
 * Actions and Action Creators
 **/

export function setConfig( config ){
	return {
		type: 'PUSHER_SET_CONFIG',
		config: config
	}
}

export function setUsername( username ){
	return {
		type: 'PUSHER_INSTRUCT',
		action: 'set_username',
		data: { username: username }
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
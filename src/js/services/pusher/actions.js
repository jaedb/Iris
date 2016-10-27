
/**
 * Actions and Action Creators
 **/

export function setConfig( config ){
	return {
		type: 'PUSHER_SET_CONFIG',
		config: config
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

export function instruct( call, value ){
	return {
		type: 'PUSHER_INSTRUCT',
		call: call,
		value: value
	}
}
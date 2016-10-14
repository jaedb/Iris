
/**
 * Actions and Action Creators
 **/

export function connect(){
	return {
		type: 'CONNECT'
	}
}

export function disconnect(){
	return {
		type: 'DISCONNECT'
	}
}

export function instruct( call, value ){
	return {
		type: 'INSTRUCT',
		call: call,
		value: value
	}
}


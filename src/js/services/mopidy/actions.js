
/**
 * Actions and Action Creators
 **/

export function connect(){
	return {
		type: 'MOPIDY_CONNECT'
	}
}

export function disconnect(){
	return {
		type: 'MOPIDY_DISCONNECT'
	}
}

export function changeTrack( tlid ){
	return {
		type: 'MOPIDY_CHANGE_TRACK',
		call: 'playback.play',
		value: { tlid: tlid }
	}
}

export function removeTracks( tlids ){
	return {
		type: 'MOPIDY_REMOVE_TRACKS',
		call: 'tracklist.remove',
		value: { tlid: tlids }
	}
}

export function instruct( call, value ){
	return {
		type: 'MOPIDY_INSTRUCT',
		call: call,
		value: value
	}
}


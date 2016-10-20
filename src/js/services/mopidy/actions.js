
/**
 * Actions and Action Creators
 **/

export function setConfig( config ){
	return {
		type: 'MOPIDY_SET_CONFIG',
		config: config
	}
}

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
		type: 'MOPIDY_INSTRUCT',
		call: 'playback.play',
		value: { tlid: tlid }
	}
}

export function playTracks( uris ){
	return {
		type: 'MOPIDY_PLAY_TRACKS',
		uris: uris
	}
}

export function enqueueTracks( uris, at_position = false ){
	if( typeof(uris) !== 'object' ) uris = [uris];
	var value = { uris: uris };
	if( at_position ) value.at_position = at_position;
	return {
		type: 'MOPIDY_INSTRUCT',
		call: 'tracklist.add',
		value: value
	}
}

export function removeTracks( tlids ){
	return {
		type: 'MOPIDY_INSTRUCT',
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

export function play(){
	return instruct('playback.play');
}

export function pause(){
	return instruct('playback.pause');
}

export function next(){
	return instruct('playback.next');
}

export function previous(){
	return instruct('playback.previous');
}

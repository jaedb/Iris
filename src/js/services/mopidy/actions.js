
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

export function instruct( call, value ){
	return {
		type: 'MOPIDY_INSTRUCT',
		call: call,
		value: value
	}
}


/**
 * Playback-oriented actions
 **/

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

export function play(){
	return instruct('playback.play');
}

export function pause(){
	return instruct('playback.pause');
}

export function stop(){
	return instruct('playback.stop');
}

export function next(){
	return instruct('playback.next');
}

export function previous(){
	return instruct('playback.previous');
}

export function seek( time_position ){
	return {
		type: 'MOPIDY_INSTRUCT',
		call: 'playback.seek',
		value: { time_position: time_position }
	}
}

export function getTimePosition(){
	return {
		type: 'MOPIDY_INSTRUCT',
		call: 'playback.getTimePosition'
	}
}



/**
 * Asset-oriented actions
 **/

export function getPlaylists(){
	return { type: 'MOPIDY_PLAYLISTS' }
}

export function getPlaylist( uri ){
	return { 
		type: 'MOPIDY_PLAYLIST', 
		data: { uri: uri } 
	}
}

export function getDirectory( uri ){
	return { 
		type: 'MOPIDY_DIRECTORY', 
		data: { uri: uri } 
	}
}

export function getAlbum( uri ){
	return { 
		type: 'MOPIDY_ALBUM', 
		data: { uri: uri } 
	}
}

export function getArtist( uri ){
	return { 
		type: 'MOPIDY_ARTIST', 
		data: { uri: uri } 
	}
}

export function getArtists(){
	return { 
		type: 'MOPIDY_ARTISTS'
	}
}

export function getAlbums(){
	return { 
		type: 'MOPIDY_ALBUMS'
	}
}


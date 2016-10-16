
/**
 * Actions and Action Creators
 **/

export function startAuthorization(){
	return {
		type: 'SPOTIFY_START_AUTHORIZATION'
	}
}

export function removeAuthorization(){
	return {
		type: 'SPOTIFY_REMOVE_AUTHORIZATION'
	}
}

export function completeAuthorization( data ){
	return {
		type: 'SPOTIFY_COMPLETE_AUTHORIZATION',
		data: data
	}
}

export function connect(){
	return {
		type: 'SPOTIFY_CONNECTING'
	}
}

export function disconnect(){
	return {
		type: 'SPOTIFY_DISCONNECT'
	}
}

export function loadAlbum( uri ){
	return {
		type: 'SPOTIFY_LOAD_ALBUM',
		uri: uri
	}
}

export function loadArtist( uri ){
	return {
		type: 'SPOTIFY_LOAD_ARTIST',
		uri: uri
	}
}


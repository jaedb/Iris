
/**
 * Actions and Action Creators
 **/

export function authorize(){
	return {
		type: 'SPOTIFY_AUTHORIZING'
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


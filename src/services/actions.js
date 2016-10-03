
/**
 * Actions and Action Creators
 **/

export const AUTHORIZE_SPOTIFY = 'AUTHORIZE_SPOTIFY'
export const AUTHORIZE_SPOTIFY_SUCCESS = 'AUTHORIZE_SPOTIFY_SUCCESS'
export const MOPIDY_ONLINE = 'MOPIDY_ONLINE'

export function authorizeSpotify( authorize = true ){
	return {
		type: AUTHORIZE_SPOTIFY,
		authorize: authorize
	}
}

export function authorizeSpotifySuccess( authorization ){
	return {
		type: AUTHORIZE_SPOTIFY_SUCCESS,
		authorization: authorization
	}
}

export function mopidyOnline( online ){
	return {
		type: MOPIDY_ONLINE,
		online: online
	}
}
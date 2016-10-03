
/**
 * Actions and Action Creators
 **/

export const MOPIDY_ONLINE = 'MOPIDY_ONLINE'
export const UPDATE_TRACKLIST = 'UPDATE_TRACKLIST'

export function mopidyOnline( online ){
	return {
		type: MOPIDY_ONLINE,
		online: online
	}
}

export function updateTracklist( tracks ){
	return {
		type: UPDATE_TRACKLIST,
		tracks: tracks
	}
}

/**
 * Actions and Action Creators
 **/

export const STATUS_CHANGED = 'STATUS_CHANGED'
export const UPDATE_TRACKLIST = 'UPDATE_TRACKLIST'
export const VOLUME_CHANGED = 'VOLUME_CHANGED'

export function updateStatus( online ){
	return {
		type: STATUS_CHANGED,
		online: online
	}
}

export function volumeChanged( volume ){
	return {
		type: VOLUME_CHANGED,
		volume: volume
	}
}

export function updateTracklist( tracks ){
	return {
		type: UPDATE_TRACKLIST,
		tracks: tracks
	}
}
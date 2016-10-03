
/**
 * Actions and Action Creators
 **/

export function updateStatus( online ){
	return {
		type: 'STATUS',
		online: online
	}
}

export function updateTracklist( tracks ){
	return {
		type: 'TRACKLIST',
		tracks: tracks
	}
}

export function updateTrackInFocus( tltrack ){
	return {
		type: 'TRACKINFOCUS',
		trackInFocus: tltrack
	}
}

export function updateVolume( volume ){
	return {
		type: 'VOLUME',
		volume: volume
	}
}
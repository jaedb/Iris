
/**
 * Actions and Action Creators
 **/

export function updateStatus( online ){
	return {
		type: 'STATUS',
		online: online
	}
}

export function updateState( state ){
	return {
		type: 'STATE',
		state: state
	}
}

export function updateConsume( consume ){
	return {
		type: 'CONSUME',
		consume: consume
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
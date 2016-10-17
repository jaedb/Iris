
/**
 * Actions and Action Creators
 **/

export function shiftKey( down ){
	return {
		type: 'SHIFT_KEY',
		down: down
	}
}

export function ctrlKey( down ){
	return {
		type: 'CTRL_KEY',
		down: down
	}
}


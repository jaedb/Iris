
/**
 * Actions and Action Creators
 **/

export const TOGGLE_DONE = 'TOGGLE_DONE'
export const ADD_TODO = 'ADD_TODO'
export const SET_ALBUM = 'SET_ALBUM'

export function toggleDone( id ){
	return {
		type: TOGGLE_DONE,
        id: id
	}
}

export function addTodo( title ){
	return {
		type: ADD_TODO,
        id: title,
        title: title
	}
}

export function setAlbum( album ){
	return {
		type: SET_ALBUM,
		album: album
	}
}



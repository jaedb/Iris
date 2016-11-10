

export function showContextMenu( e, context = false, data ){
    return {
        type: 'SHOW_CONTEXT_MENU',
        position_x: e.clientX,
        position_y: e.clientY,
        context: context,
        data: data
    }
}

export function hideContextMenu(){
    return {
        type: 'HIDE_CONTEXT_MENU'
    }
}

export function searchStarted(){
	return {
		type: 'SEARCH_STARTED',
        data: {
            artists: [],
            albums: [],
            playlists: [],
            tracks: [],
            artists_more: false,
            albums_more: false,
            playlists_more: false,
            tracks_more: false
        }
	}
}

export function lazyLoading( start ){
    return {
        type: 'LAZY_LOADING',
        start: start
    }
}

export function dragStart( e, context, victims ){
    return {
        type: 'DRAG_START',
        context: context,
        victims: victims,
        position_x: e.clientX,
        position_y: e.clientY
    }
}

export function dragMove( e ){
    return {
        type: 'DRAG_MOVE',
        position_x: e.clientX,
        position_y: e.clientY,
    }
}

export function dragCancel(){
    return { type: 'DRAG_CANCEL' }
}

export function dragEnd(){
    return { type: 'DRAG_END' }
}



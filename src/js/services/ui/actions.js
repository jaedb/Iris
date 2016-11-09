

export function showContextMenu( e, context = false, data ){
    return {
        type: 'UI_SHOW_CONTEXT_MENU',
        position_x: e.clientX,
        position_y: e.clientY,
        context: context,
        data: data
    }
}

export function hideContextMenu(){
    return {
        type: 'UI_HIDE_CONTEXT_MENU'
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
        type: 'UI_LAZY_LOADING',
        start: start
    }
}



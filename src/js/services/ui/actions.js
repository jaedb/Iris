

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

export function setCurrentTrack( track ){
    return {
        type: 'UI_CURRENT_TRACK',
        track: track
    }
}

export function lazyLoading( start ){
    return {
        type: 'UI_LAZY_LOADING',
        start: start
    }
}



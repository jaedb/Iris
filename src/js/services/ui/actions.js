

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



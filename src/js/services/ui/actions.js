
import * as helpers from '../../helpers'

export function setWindowTitle(title = null, play_state = null){
    return {
        type: 'SET_WINDOW_TITLE',
        title: title,
        play_state: play_state
    }
}

export function setSelectedTracks(keys = []){
    if (typeof(keys) === 'string'){
        keys = [keys]
    }
    return {
        type: 'SET_SELECTED_TRACKS',
        keys: keys
    }
}

export function showContextMenu(data){

    // Touchend
    if (data.e.changedTouches){
        data.position_x = data.e.changedTouches[0].clientX;
        data.position_y = data.e.changedTouches[0].clientY;

    // Touchstart
    } else if (data.e.touches){
        data.position_x = data.e.touches[0].clientX;
        data.position_y = data.e.touches[0].clientY;

    // Click/mousedown/mouseup/etc
    } else {
        data.position_x = data.e.clientX;
        data.position_y = data.e.clientY;
    }

    return {
        type: 'SHOW_CONTEXT_MENU',
        data: data
    }
}

export function setSlimMode(slim_mode){
    return {
        type: 'SET_SLIM_MODE',
        slim_mode: slim_mode
    }
}

export function hideContextMenu(){
    return {
        type: 'HIDE_CONTEXT_MENU'
    }
}

export function removeContextMenu(){
    return {
        type: 'REMOVE_CONTEXT_MENU'
    }
}

export function showTouchContextMenu(data){
    return {
        type: 'SHOW_TOUCH_CONTEXT_MENU',
        data: data
    }
}

export function hideTouchContextMenu(){
    return {
        type: 'HIDE_TOUCH_CONTEXT_MENU'
    }
}

export function removeTouchContextMenu(){
    return {
        type: 'REMOVE_TOUCH_CONTEXT_MENU'
    }
}

export function lazyLoading(start){
    return {
        type: 'LAZY_LOADING',
        start: start
    }
}

export function toggleSidebar(new_state = 'toggle'){
    var action = {
        type: 'TOGGLE_SIDEBAR'
    }
    if (new_state != 'toggle'){
        action.new_state = new_state
    }
    return action
}

export function dragStart(e, context, from_uri = null, victims, victims_indexes = null){
    return {
        type: 'DRAG_START',
        context: context,
        from_uri: from_uri,
        victims: victims,
        victims_indexes: victims_indexes,
        start_x: e.clientX,
        start_y: e.clientY
    }
}

export function dragActive(){
    return { type: 'DRAG_ACTIVE' }
}

export function dragEnd(){
    return { type: 'DRAG_END' }
}

export function set(data){
    return {
        type: 'UI_SET',
        data: data
    }
}

export function installPrompt(event){
    return {
        type: 'INSTALL_PROMPT',
        event: event
    }
}



/**
 * Notifications
 *
 * Subtle info/tooltip messages
 **/

export function createBrowserNotification(data){
    return { 
        type: 'BROWSER_NOTIFICATION',
        data: data
    }
}

export function createNotification(data){

	// Allow 'message' as an alias of our content
	if (data.message){
		data.content = data.message;
	}

    return { 
        type: 'CREATE_NOTIFICATION',
        notification: Object.assign(
            {
                key: helpers.generateGuid(),
                duration: 5,
                type: 'default',
                title: null,
                content: null,
                description: null,
                sticky: false,
                closing: false,
                render_content: false
            },
            data
        )
    }
}

export function closeNotification(key){
    return { 
        type: 'CLOSE_NOTIFICATION',
        key: key
    }
}

export function removeNotification(key, manual = false){
    return { 
        type: 'REMOVE_NOTIFICATION',
        key: key,
        manual: manual
    }
}



/**
 * Loaders
 **/

export function startLoading(key,source){
    return {
        type: 'START_LOADING',
        source: source,
        key: key
    }
}

export function stopLoading(key){
    return { 
        type: 'STOP_LOADING',
        key: key
    }
}

export function startProcess(key,message,data = {}){
    return { 
        type: 'START_PROCESS',
        key: key,
        message: message,
        data: data
    }
}

export function resumeProcess(key,message,data = {}){
    return { 
        type: 'RESUME_PROCESS',
        key: key
    }
}

export function updateProcess(key,message,data = {}){
    return { 
        type: 'UPDATE_PROCESS',
        key: key,
        message: message,
        data: data
    }
}

export function runProcess(key,data = {}){
    return { 
        type: key,
        data: data
    }
}

export function cancelProcess(key){
    return { 
        type: 'CANCEL_PROCESS',
        key: key
    }
}

export function processCancelled(key){
    return { 
        type: 'PROCESS_CANCELLED',
        key: key
    }
}

export function processFinishing(key){
    return { 
        type: 'PROCESS_FINISHING',
        key: key
    }
}

export function processFinished(key){
    return { 
        type: 'PROCESS_FINISHED',
        key: key
    }
}

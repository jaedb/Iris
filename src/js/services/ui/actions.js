
import * as helpers from '../../helpers'

export function showContextMenu(data){
    data.position_x = data.e.clientX
    data.position_y = data.e.clientY
    return {
        type: 'SHOW_CONTEXT_MENU',
        data: data
    }
}

export function hideContextMenu(){
    return {
        type: 'HIDE_CONTEXT_MENU'
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

export function startSearch( query ){
	return {
		type: 'SEARCH_STARTED',
        query: query
	}
}

export function debugResponse( response ){
    return {
        type: 'DEBUG',
        response: response
    }
}

export function lazyLoading( start ){
    return {
        type: 'LAZY_LOADING',
        start: start
    }
}

export function toggleSidebar( new_state = 'toggle' ){
    var action = { type: 'TOGGLE_SIDEBAR' }
    if( new_state != 'toggle' ) action.new_state = new_state
    return action
}

export function dragStart( e, context, from_uri = null, victims, victims_indexes = null ){
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

export function set( data ){
    return {
        type: 'UI_SET',
        data: data
    }
}

export function reorderPlaylistTracks( uri, indexes, insert_before, snapshot_id = false ){
    var range = helpers.createRange( indexes );
    switch( helpers.uriSource( uri ) ){

        case 'spotify':
            return { 
                type: 'SPOTIFY_REORDER_PLAYLIST_TRACKS',
                key: uri,
                range_start: range.start,
                range_length: range.length,
                insert_before: insert_before,
                snapshot_id: snapshot_id
            }

        case 'm3u':
            return { 
                type: 'MOPIDY_REORDER_PLAYLIST_TRACKS',
                key: uri,
                range_start: range.start,
                range_length: range.length,
                insert_before: insert_before
            }
    }
}

export function savePlaylist( uri, name, is_public = false ){
    switch( helpers.uriSource( uri ) ){

        case 'spotify':
            return { 
                type: 'SPOTIFY_SAVE_PLAYLIST',
                key: uri,
                name: name,
                is_public: is_public
            }

        case 'm3u':
            return { 
                type: 'MOPIDY_SAVE_PLAYLIST',
                key: uri,
                name: name
            }
    }
    return false
}

export function createPlaylist( scheme, name, is_public = false ){
    switch( scheme ){

        case 'spotify':
            return { 
                type: 'SPOTIFY_CREATE_PLAYLIST',
                name: name,
                is_public: is_public
            }

        case 'm3u':
            return { 
                type: 'MOPIDY_CREATE_PLAYLIST',
                scheme: scheme,
                name: name
            }
    }
    return false
}

export function removeTracksFromPlaylist( uri, tracks_indexes ){
    switch( helpers.uriSource( uri ) ){

        case 'spotify':
            return { 
                type: 'SPOTIFY_REMOVE_PLAYLIST_TRACKS',
                key: uri,
                tracks_indexes: tracks_indexes
            }

        case 'm3u':
            return { 
                type: 'MOPIDY_REMOVE_PLAYLIST_TRACKS',
                key: uri,
                tracks_indexes: tracks_indexes
            }
    }
}

export function addTracksToPlaylist( uri, tracks_uris ){
    switch( helpers.uriSource( uri ) ){

        case 'spotify':
            return { 
                type: 'SPOTIFY_ADD_PLAYLIST_TRACKS',
                key: uri,
                tracks_uris: tracks_uris
            }

        case 'm3u':
            return { 
                type: 'MOPIDY_ADD_PLAYLIST_TRACKS',
                key: uri,
                tracks_uris: tracks_uris
            }
    }
}


/**
 * Modal
 *
 * Immersive full-screen dialog
 **/

export function openModal( name, data ){
    return { 
        type: 'OPEN_MODAL',
        modal: {
            name: name,
            data: data
        }
    }
}

export function closeModal(){
    return { type: 'CLOSE_MODAL' }
}


/**
 * Notifications
 *
 * Subtle info/tooltip messages
 **/

export function createBrowserNotification( data ){
    return { 
        type: 'BROWSER_NOTIFICATION',
        data: data
    }
}

export function createNotification( content, type = 'default' ){
    return { 
        type: 'CREATE_NOTIFICATION',
        notification: {
            id: helpers.generateGuid(),
            type: type,
            content: content
        }
    }
}

export function removeNotification( id ){
    return { 
        type: 'REMOVE_NOTIFICATION',
        id: id
    }
}




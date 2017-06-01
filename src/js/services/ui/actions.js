
import * as helpers from '../../helpers'

export function getBroadcasts(){
    return (dispatch, getState) => {
        var config = {
            method: 'GET',
            url: 'https://gist.githubusercontent.com/jaedb/b677dccf80daf3ccb2ef12e96e495677/raw'
        }
        $.ajax(config).then( 
                response => {
                    dispatch({
                        type: 'BROADCASTS_LOADED',
                        broadcasts: JSON.parse(response)
                    })
                },
                (xhr, status, error) => {
                    console.error('Could not fetch broadcasts from GitHub')
                }
            )
    }
}

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

export function startSearch(search_type, query, only_mopidy = false){
	return {
		type: 'SEARCH_STARTED',
        search_type: search_type,
        query: query,
        only_mopidy: only_mopidy
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
 * Assets loaded
 **/

export function albumLoaded(key,album){
    return {
        type: 'ALBUM_LOADED',
        key: key,
        album: album
    }
}

export function albumsLoaded(albums){
    return {
        type: 'ALBUMS_LOADED',
        albums: albums
    }
}

export function artistLoaded(key,artist){
    return {
        type: 'ARTIST_LOADED',
        key: key,
        artist: artist
    }
}

export function artistsLoaded(artists){
    return {
        type: 'ALBUMS_LOADED',
        artists: artists
    }
}

export function trackLoaded(key,track){
    return {
        type: 'TRACK_LOADED',
        key: key,
        track: track
    }
}

export function tracksLoaded(tracks){
    return {
        type: 'TRACKS_LOADED',
        tracks: tracks
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

export function createNotification(content, type = 'default', key = null, title = null, sticky = false){
    if (!key){
        key = helpers.generateGuid()
    }
    return { 
        type: 'CREATE_NOTIFICATION',
        notification: {
            key: key,
            type: type,
            title: title,
            content: content,
            sticky: sticky
        }
    }
}

export function removeNotification(key){
    return { 
        type: 'REMOVE_NOTIFICATION',
        key: key
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

export function startProcess(key,content){
    return { 
        type: 'START_PROCESS',
        key: key,
        content: content
    }
}

export function cancelProcess(key){
    return { 
        type: 'CANCEL_PROCESS',
        key: key
    }
}

export function stopProcess(key){
    return { 
        type: 'STOP_PROCESS',
        key: key
    }
}

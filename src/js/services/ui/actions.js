
import * as helpers from '../../helpers'

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

export function toggleSidebar(){
    return {
        type: 'TOGGLE_SIDEBAR'
    }
}

export function dragStart( e, context, victims, victims_indexes = false ){
    return {
        type: 'DRAG_START',
        context: context,
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

export function setView( view ){
    return {
        type: 'SET_VIEW',
        view: view
    }
}

export function reorderPlaylistTracks( uri, indexes, insert_before, snapshot_id = false ){
    var range = helpers.createRange( indexes );
    switch( helpers.uriSource( uri ) ){

        case 'spotify':
            return { 
                type: 'SPOTIFY_REORDER_PLAYLIST_TRACKS',
                uri: uri,
                range_start: range.start,
                range_length: range.length,
                insert_before: insert_before,
                snapshot_id: snapshot_id
            }

        case 'm3u':
            return { 
                type: 'MOPIDY_REORDER_PLAYLIST_TRACKS',
                uri: uri,
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
                uri: uri,
                name: name,
                is_public: is_public
            }

        case 'm3u':
            return { 
                type: 'MOPIDY_SAVE_PLAYLIST',
                uri: uri,
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

export function removeTracksFromPlaylist( playlist_uri, tracks_indexes ){
    switch( helpers.uriSource( playlist_uri ) ){

        case 'spotify':
            return { 
                type: 'SPOTIFY_REMOVE_PLAYLIST_TRACKS',
                playlist_uri: playlist_uri,
                tracks_indexes: tracks_indexes
            }

        case 'm3u':
            return { 
                type: 'MOPIDY_REMOVE_PLAYLIST_TRACKS',
                playlist_uri: playlist_uri,
                tracks_indexes: tracks_indexes
            }
    }
}

export function addTracksToPlaylist( playlist_uri, tracks_uris ){
    switch( helpers.uriSource( playlist_uri ) ){

        case 'spotify':
            return { 
                type: 'SPOTIFY_ADD_PLAYLIST_TRACKS',
                playlist_uri: playlist_uri,
                tracks_uris: tracks_uris
            }

        case 'm3u':
            return { 
                type: 'MOPIDY_ADD_PLAYLIST_TRACKS',
                playlist_uri: playlist_uri,
                tracks_uris: tracks_uris
            }
    }
}

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


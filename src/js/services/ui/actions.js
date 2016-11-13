
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

export function createPlaylist( scheme, name, is_private = false ){
    switch( scheme ){

        case 'spotify':
            return { 
                type: 'SPOTIFY_CREATE_PLAYLIST',
                name: name,
                is_private: is_private
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

export function addTracksToPlaylist( playlist_uri, tracks ){

    var tracks_uris = []
    for( var i = 0; i < tracks.length; i++ ){
        tracks_uris.push( tracks[i].uri )
    }

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


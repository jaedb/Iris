
import * as helpers from '../../helpers'
var spotifyActions = require('../../services/spotify/actions')
var mopidyActions = require('../../services/mopidy/actions')

export function getBroadcasts(){
    return (dispatch, getState) => {
        var config = {
            method: 'GET',
            timeout: 15000,
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
                    dispatch(
                        handleException(
                            'Could not fetch broadcasts from GitHub',
                            {
                                config: config,
                                xhr: xhr,
                                status: status,
                                error: error
                            }
                        )
                    );
                }
            )
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

export function handleException(message, data = {}, description = null){
    return {
        type: 'HANDLE_EXCEPTION',
        message: message,
        description: description,
        data: data
    }
}

export function debugResponse(response){
    return {
        type: 'DEBUG',
        response: response
    }
}


export function startServices(){
    return {
        type: 'CORE_START_SERVICES'
    }
}

export function set(data){
    return {
        type: 'CORE_SET',
        data: data
    }
}




/**
 * Playlist manipulation
 **/

export function reorderPlaylistTracks(uri, indexes, insert_before, snapshot_id = false){
    var range = helpers.createRange(indexes);
    switch(helpers.uriSource(uri)){

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

export function savePlaylist(uri, name, description = '', is_public = false, is_collaborative = false){
    switch (helpers.uriSource(uri)){

        case 'spotify':
            return { 
                type: 'SPOTIFY_SAVE_PLAYLIST',
                key: uri,
                name: name,
                description: (description == '' ? null : description),
                is_public: is_public,
                is_collaborative: is_collaborative
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

export function createPlaylist(scheme, name, description = '', is_public = false, is_collaborative = false){
    switch (scheme){

        case 'spotify':
            if (description == ''){
                description = null
            }
            return spotifyActions.createPlaylist(name, description, is_public, is_collaborative )

        default:
            return mopidyActions.createPlaylist(name, scheme)
    }
    return false
}

export function deletePlaylist(uri){
    switch (helpers.uriSource(uri)){

        case 'spotify':
            return spotifyActions.following(uri, 'DELETE')

        default:
            return mopidyActions.deletePlaylist(uri)
    }
    return false
}

export function removeTracksFromPlaylist(uri, tracks_indexes){
    switch(helpers.uriSource(uri )){

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

export function addTracksToPlaylist(uri, tracks_uris){
    switch(helpers.uriSource(uri )){

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
 * Asset libraries
 **/

export function getLibraryPlaylists(){
    return {
        type: 'GET_LIBRARY_PLAYLISTS'
    }
}

export function getLibraryAlbums(){
    return {
        type: 'GET_LIBRARY_ALBUMS'
    }
}

export function getLibraryArtists(){
    return {
        type: 'GET_LIBRARY_ARTISTS'
    }
}


/**
 * Assets loaded
 **/

export function loadedMore(parent_type, parent_key, records_type, records_data){
    return {
        type: 'LOADED_MORE',
        parent_type: parent_type,
        parent_key: parent_key,
        records_type: records_type,
        records_data: records_data
    }
}

export function tracksLoaded(tracks){
    return {
        type: 'TRACKS_LOADED',
        tracks: tracks
    }
}

export function albumsLoaded(albums){
    return {
        type: 'ALBUMS_LOADED',
        albums: albums
    }
}

export function artistsLoaded(artists){
    return {
        type: 'ARTISTS_LOADED',
        artists: artists
    }
}

export function playlistsLoaded(playlists){
    return {
        type: 'PLAYLISTS_LOADED',
        playlists: playlists
    }
}

export function usersLoaded(users){
    return {
        type: 'USERS_LOADED',
        users: users
    }
}

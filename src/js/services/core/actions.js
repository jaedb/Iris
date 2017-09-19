
import * as helpers from '../../helpers'
var spotifyActions = require('../../services/spotify/actions')
var mopidyActions = require('../../services/mopidy/actions')

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

export function startSearch(search_type, query, only_mopidy = false){
	return {
		type: 'SEARCH_STARTED',
        search_type: search_type,
        query: query,
        only_mopidy: only_mopidy
	}
}

export function handleException(message, data = {}){
    return {
        type: 'HANDLE_EXCEPTION',
        message: message,
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

export function set( data ){
    return {
        type: 'CORE_SET',
        data: data
    }
}




/**
 * Playlist manipulation
 **/

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
            return spotifyActions.createPlaylist( name, description, is_public, is_collaborative )

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

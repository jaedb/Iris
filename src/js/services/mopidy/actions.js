
import * as helpers from '../../helpers'

export function setConfig( config ){
	return {
		type: 'MOPIDY_SET_CONFIG',
		config: config
	}
}

export function connect(){
	return {
		type: 'MOPIDY_CONNECT'
	}
}

export function disconnect(){
	return {
		type: 'MOPIDY_DISCONNECT'
	}
}

export function instruct( call, value ){
	return {
		type: 'MOPIDY_INSTRUCT',
		call: call,
		value: value
	}
}

export function debug( call, value ){
	return {
		type: 'MOPIDY_DEBUG',
		call: call,
		value: value
	}
}


/**
 * Playback-oriented actions
 **/

export function changeTrack( tlid ){
	return {
		type: 'MOPIDY_INSTRUCT',
		call: 'playback.play',
		value: { tlid: tlid }
	}
}

export function playURIs( uris ){
	return {
		type: 'MOPIDY_PLAY_URIS',
		uris: uris
	}
}

export function enqueueTracksNext( uris ){
	if( typeof(uris) !== 'object' ) uris = [uris]
	return {
		type: 'MOPIDY_ENQUEUE_TRACKS_NEXT',
		uris: uris
	}
}

export function enqueueTracks( uris, at_position = false ){
	if( typeof(uris) !== 'object' ) uris = [uris];
	var value = { uris: uris };
	if( at_position ) value.at_position = at_position;
	return {
		type: 'MOPIDY_INSTRUCT',
		call: 'tracklist.add',
		value: value
	}
}

export function removeTracks( tlids ){
	return {
		type: 'MOPIDY_INSTRUCT',
		call: 'tracklist.remove',
		value: { tlid: tlids }
	}
}

export function reorderTracklist( indexes, insert_before ){
	var range = helpers.createRange( indexes );
	if( insert_before > range.start ) insert_before = insert_before - range.length
	return { 
		type: 'MOPIDY_REORDER_TRACKLIST',
		range_start: range.start,
		range_length: range.length,
		insert_before: insert_before
	}
}

export function play(){
	return instruct('playback.play');
}

export function pause(){
	return instruct('playback.pause');
}

export function stop(){
	return instruct('playback.stop');
}

export function next(){
	return instruct('playback.next');
}

export function previous(){
	return instruct('playback.previous');
}

export function setVolume( volume ){
	return {
		type: 'MOPIDY_INSTRUCT',
		call: 'playback.setVolume',
		value: { volume: volume }
	}
}

export function seek( time_position ){
	return {
		type: 'MOPIDY_INSTRUCT',
		call: 'playback.seek',
		value: { time_position: time_position }
	}
}

export function getTimePosition(){
	return {
		type: 'MOPIDY_INSTRUCT',
		call: 'playback.getTimePosition'
	}
}

export function setTimePosition( time_position ){
	return {
		type: 'MOPIDY_TIMEPOSITION',
		data: time_position
	}
}



/**
 * Asset-oriented actions
 **/

export function deletePlaylist( uri ){
	return { 
		type: 'MOPIDY_DELETE_PLAYLIST',
		uri: uri
	}
}

export function getLibraryPlaylists(){
	return { type: 'MOPIDY_GET_LIBRARY_PLAYLISTS' }
}

export function getPlaylist( uri ){
	return { 
		type: 'MOPIDY_GET_PLAYLIST', 
		data: { uri: uri } 
	}
}

export function getDirectory( uri ){
	return { 
		type: 'MOPIDY_GET_DIRECTORY', 
		data: { uri: uri } 
	}
}

export function getArtist( uri ){
	return { 
		type: 'MOPIDY_GET_ARTIST', 
		data: { uri: uri } 
	}
}

export function getArtists(){
	return { 
		type: 'MOPIDY_GET_ARTISTS'
	}
}

export function getAlbum( uri ){
	return { 
		type: 'MOPIDY_GET_ALBUM', 
		data: { uri: uri } 
	}
}

export function getAlbums(){
	return { 
		type: 'MOPIDY_GET_ALBUMS'
	}
}


/**
 * Other general actions
 **/

export function getSearchResults(query, backends = null, fields = ['any']){
	var queryObj = {};
	for( var i = 0; i < fields.length; i++ ){
		queryObj[fields[i]] = [query];
	}

	return {
		type: 'MOPIDY_INSTRUCT',
		call: 'library.search',
		value: { query: queryObj, uris: backends }
	}
}
 
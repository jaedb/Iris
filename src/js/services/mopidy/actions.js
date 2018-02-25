
import * as helpers from '../../helpers'

export function setConfig(config){
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

export function instruct(call, value){
	return {
		type: 'MOPIDY_INSTRUCT',
		call: call,
		value: value
	}
}

export function debug(call, value){
	return {
		type: 'MOPIDY_DEBUG',
		call: call,
		value: value
	}
}


/**
 * Playback-oriented actions
 **/

export function changeTrack(tlid){
	return {
		type: 'MOPIDY_CHANGE_TRACK',
		tlid: tlid
	}
}

export function playURIs(uris, from_uri = null){
	return {
		type: 'MOPIDY_PLAY_URIS',
		uris: uris,
		from_uri: from_uri
	}
}

export function enqueueURIs(uris, from_uri = null, next = false, at_position = null, offset = 0){
	return {
		type: 'MOPIDY_ENQUEUE_URIS',
		uris: uris,
		at_position: at_position,
		next: next,
		from_uri: from_uri,
		offset: offset
	}
}

export function enqueueURIsBatchDone(){
	return {
		type: 'MOPIDY_ENQUEUE_URIS_BATCH_DONE'
	}
}

export function playPlaylist(uri){
	return {
		type: 'MOPIDY_PLAY_PLAYLIST',
		uri: uri
	}
}

export function playAlbum(uri){
	return {
		type: 'MOPIDY_PLAY_ALBUM',
		uri: uri
	}
}

export function removeTracks(tlids){
	return {
		type: 'MOPIDY_REMOVE_TRACKS',
		tlids: tlids
	}
}

export function reorderTracklist(indexes, insert_before){
	var range = helpers.createRange(indexes );
	if (insert_before > range.start ) insert_before = insert_before - range.length
	return { 
		type: 'MOPIDY_REORDER_TRACKLIST',
		range_start: range.start,
		range_length: range.length,
		insert_before: insert_before
	}
}

export function clearTracklist(){
	return instruct('tracklist.clear')
}

export function play(){
	return {
		type: 'MOPIDY_TRIGGER_PLAY'
	}
}

export function pause(){
	return instruct('playback.pause')
}

export function stop(){
	return instruct('playback.stop')
}

export function next(){
	return instruct('playback.next')
}

export function previous(){
	return instruct('playback.previous')
}

export function setMute(mute){
	return instruct('mixer.setMute', {mute: mute})
}

export function setVolume(volume){
	return instruct('playback.setVolume', {volume: volume})
}

export function seek(time_position){
	return instruct('playback.seek', {time_position: parseInt(time_position)})
}

export function getTimePosition(){
	return instruct('playback.getTimePosition')
}

export function setTimePosition(time_position){
	return {
		type: 'MOPIDY_TIMEPOSITION',
		data: time_position
	}
}



/**
 * Asset-oriented actions
 **/

export function getImages(context, uris){
	return { 
		type: 'MOPIDY_GET_IMAGES',
		context: context,
		uris: uris
	}
}

export function createPlaylist(name, scheme){
	return { 
		type: 'MOPIDY_CREATE_PLAYLIST',
		name: name,
		scheme: scheme
	}
}

export function deletePlaylist(uri){
	return { 
		type: 'MOPIDY_DELETE_PLAYLIST',
		uri: uri
	}
}

export function getLibraryPlaylists(){
	return { type: 'MOPIDY_GET_LIBRARY_PLAYLISTS' }
}

export function getPlaylist(uri){
	return { 
		type: 'MOPIDY_GET_PLAYLIST', 
		data: { uri: uri } 
	}
}

export function getPlaylists(uris, processor = null){
	return { 
		type: 'MOPIDY_GET_PLAYLISTS', 
		uris: uris,
		processor: processor
	}
}

export function getDirectory(uri){
	return { 
		type: 'MOPIDY_GET_DIRECTORY', 
		data: { uri: uri } 
	}
}

export function getTrack(uri){
	return { 
		type: 'MOPIDY_GET_TRACK', 
		data: { uri: uri } 
	}
}

export function getLibraryArtists(){
	return { 
		type: 'MOPIDY_GET_LIBRARY_ARTISTS' 
	}
}

export function getArtist(uri){
	return { 
		type: 'MOPIDY_GET_ARTIST', 
		data: { uri: uri } 
	}
}

export function getArtists(uris, processor = null){
	return { 
		type: 'MOPIDY_GET_ARTISTS', 
		uris: uris,
		processor: processor
	}
}

export function getAlbum(uri){
	return { 
		type: 'MOPIDY_GET_ALBUM', 
		data: { uri: uri } 
	}
}

export function getAlbums(uris, processor = null){
	return { 
		type: 'MOPIDY_GET_ALBUMS', 
		uris: uris,
		processor: processor
	}
}

export function getLibraryAlbums(){
	return { 
		type: 'MOPIDY_GET_LIBRARY_ALBUMS'
	}
}

export function runProcessor(processor){
	return {
		type: processor
	}
}

export function cancelProcessor(processor){
	return {
		type: processor+'_CANCEL'
	}
}


/**
 * Searching
 **/

export function clearSearchResults(){
    return {
        type: 'MOPIDY_CLEAR_SEARCH_RESULTS'
    }
}

export function getSearchResults(context, query, limit = 100){
	return {
		type: 'MOPIDY_GET_SEARCH_RESULTS',
		context: context,
		query: query,
		limit: limit
	}
}


/**
 * Other general actions
 **/

export function getQueueHistory(){
	return instruct('history.getHistory')
}
 
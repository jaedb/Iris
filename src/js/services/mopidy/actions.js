
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

export function connecting(){
	return {
		type: 'MOPIDY_CONNECTING'
	}
}

export function upgradeStarted(){
	return {
		type: 'MOPIDY_UPGRADE_STARTED'
	}
}

export function restartStarted(){
	return {
		type: 'MOPIDY_RESTART_STARTED'
	}
}

export function disconnect(){
	return {
		type: 'MOPIDY_DISCONNECT'
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
 * Core play actions
 **/

export function getPlayState(){
	return {
		type: 'MOPIDY_GET_PLAY_STATE'
	}
}

export function play(){
	return {
		type: 'MOPIDY_PLAY'
	}
}

export function pause(){
	return {
		type: 'MOPIDY_PAUSE'
	}
}

export function stop(){
	return {
		type: 'MOPIDY_PAUSE'
	}
}

export function next(){
	return {
		type: 'MOPIDY_NEXT'
	}
}

export function previous(){
	return {
		type: 'MOPIDY_PREVIOUS'
	}
}

export function getMute(){
	return {
		type: 'MOPIDY_GET_MUTE'
	}
}

export function setMute(mute){
	return {
		type: 'MOPIDY_SET_MUTE',
		mute: mute
	}
}

export function getVolume(){
	return {
		type: 'MOPIDY_GET_VOLUME'
	}
}

export function setVolume(volume){
	return {
		type: 'MOPIDY_SET_VOLUME',
		volume: volume
	}
}

export function getConsume(){
	return {
		type: 'MOPIDY_GET_CONSUME'
	}
}

export function setConsume(consume){
	return {
		type: 'MOPIDY_SET_CONSUME',
		consume: consume
	}
}

export function getRepeat(){
	return {
		type: 'MOPIDY_GET_REPEAT'
	}
}

export function setRepeat(repeat){
	return {
		type: 'MOPIDY_SET_REPEAT',
		repeat: repeat
	}
}

export function getRandom(){
	return {
		type: 'MOPIDY_GET_RANDOM'
	}
}

export function setRandom(random){
	return {
		type: 'MOPIDY_SET_RANDOM',
		random: random
	}
}

export function getTimePosition(){
	return {
		type: 'MOPIDY_GET_TIME_POSITION'
	}
}

export function setTimePosition(time_position){
	return {
		type: 'MOPIDY_SET_TIME_POSITION',
		time_position: parseInt(time_position)
	}
}

export function timePosition(time_position){
	return {
		type: 'MOPIDY_TIME_POSITION',
		time_position: parseInt(time_position)
	}
}

export function getUriSchemes(){
	return {
		type: 'MOPIDY_GET_URI_SCHEMES'
	}
}



/**
 * Advanced playback actions
 **/

export function getCurrentTrack(){
	return {
		type: 'MOPIDY_GET_CURRENT_TRACK'
	}
}

export function currentTrackLoaded(tl_track){
	return {
		type: 'MOPIDY_CURRENT_TRACK_LOADED',
		tl_track: tl_track
	}
}

export function getNextTrack(){
	return {
		type: 'MOPIDY_GET_NEXT_TRACK'
	}
}

export function clearCurrentTrack(){
	return {
		type: 'CURRENT_TRACK_LOADED',
        track: null,
        uri: null
	}
}

export function getQueue(){
	return {
		type: 'MOPIDY_GET_QUEUE'
	}
}

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
	return { 
		type: 'MOPIDY_CLEAR_TRACKLIST'
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
	return {
		type: 'MOPIDY_GET_QUEUE_HISTORY'
	}
}
 
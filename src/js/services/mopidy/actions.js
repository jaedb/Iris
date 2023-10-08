import { createRange } from '../../util/arrays';
import { generateGuid } from '../../util/helpers';

export function set(data) {
  return {
    type: 'MOPIDY_SET',
    data,
  };
}

export function updateServer(server) {
  return {
    type: 'MOPIDY_UPDATE_SERVER',
    server,
  };
}

export function updateServers(servers) {
  return {
    type: 'MOPIDY_UPDATE_SERVERS',
    servers,
  };
}

export function addServer(data) {
  const {
    hostname: host,
    port,
    protocol,
  } = window.location;
  const server = {
    host,
    ssl: protocol === 'https:',
    port: port || (protocol === 'https:' ? '443' : '80'),
    ...data,
  };
  return {
    type: 'MOPIDY_UPDATE_SERVER',
    server: {
      ...server,
      id: generateGuid(),
      name: `${server.host}${server.ssl ? ' 🔒' : ''}`,
    },
  };
}

export function setCurrentServer(server) {
  return {
    type: 'MOPIDY_SET_CURRENT_SERVER',
    server,
  };
}

export function getServerState(id) {
  return {
    type: 'MOPIDY_GET_SERVER_STATE',
    id,
  };
}

export function removeServer(id) {
  return {
    type: 'MOPIDY_REMOVE_SERVER',
    id,
  };
}

export function setConnection(data) {
  return {
    type: 'MOPIDY_SET_CONNECTION',
    data,
  };
}

export function request(method, params = {}, response_callback = null, error_callback = null) {
  return {
    type: 'MOPIDY_REQUEST',
    method,
    params,
    response_callback,
    error_callback,
  };
}

export function connect() {
  return {
    type: 'MOPIDY_CONNECT',
  };
}

export function connecting() {
  return {
    type: 'MOPIDY_CONNECTING',
  };
}

export function disconnect() {
  return {
    type: 'MOPIDY_DISCONNECT',
  };
}

export function debug(call, value) {
  return {
    type: 'MOPIDY_DEBUG',
    call,
    value,
  };
}

export function restartStarted() {
  return {
    type: 'MOPIDY_RESTART_STARTED',
  };
}

export function restartFinished() {
  return {
    type: 'MOPIDY_RESTART_FINISHED',
  };
}

export function upgradeStarted() {
  return {
    type: 'MOPIDY_UPGRADE_STARTED',
  };
}

export function upgradeFinished() {
  return {
    type: 'MOPIDY_UPGRADE_FINISHED',
  };
}

export function localScanStarted() {
  return {
    type: 'MOPIDY_LOCAL_SCAN_STARTED',
  };
}

export function localScanFinished() {
  return {
    type: 'MOPIDY_LOCAL_SCAN_FINISHED',
  };
}

/**
 * Core play actions
 * */

export function getPlayState() {
  return {
    type: 'MOPIDY_GET_PLAY_STATE',
  };
}

export function play() {
  return {
    type: 'MOPIDY_PLAY',
  };
}

export function pause() {
  return {
    type: 'MOPIDY_PAUSE',
  };
}

export function stop() {
  return {
    type: 'MOPIDY_STOP',
  };
}

export function next() {
  return {
    type: 'MOPIDY_NEXT',
  };
}

export function previous() {
  return {
    type: 'MOPIDY_PREVIOUS',
  };
}

export function getMute() {
  return {
    type: 'MOPIDY_GET_MUTE',
  };
}

export function setMute(mute) {
  return {
    type: 'MOPIDY_SET_MUTE',
    mute,
  };
}

export function getVolume() {
  return {
    type: 'MOPIDY_GET_VOLUME',
  };
}

export function setVolume(volume) {
  return {
    type: 'MOPIDY_SET_VOLUME',
    volume,
  };
}

export function getConsume() {
  return {
    type: 'MOPIDY_GET_CONSUME',
  };
}

export function setConsume(consume) {
  return {
    type: 'MOPIDY_SET_CONSUME',
    consume,
  };
}

export function getRepeat() {
  return {
    type: 'MOPIDY_GET_REPEAT',
  };
}

export function setRepeat(repeat) {
  return {
    type: 'MOPIDY_SET_REPEAT',
    repeat,
  };
}

export function getRandom() {
  return {
    type: 'MOPIDY_GET_RANDOM',
  };
}

export function setRandom(random) {
  return {
    type: 'MOPIDY_SET_RANDOM',
    random,
  };
}

export function getTimePosition() {
  return {
    type: 'MOPIDY_GET_TIME_POSITION',
  };
}

export function setTimePosition(time_position) {
  return {
    type: 'MOPIDY_SET_TIME_POSITION',
    time_position: parseInt(time_position),
  };
}

export function timePosition(time_position) {
  return {
    type: 'MOPIDY_TIME_POSITION',
    time_position: parseInt(time_position),
  };
}

export function getUriSchemes() {
  return {
    type: 'MOPIDY_GET_URI_SCHEMES',
  };
}

/**
 * Advanced playback actions
 * */

export function getCurrentTrack() {
  return {
    type: 'MOPIDY_GET_CURRENT_TRACK',
  };
}

export function getStreamTitle() {
  return {
    type: 'MOPIDY_GET_STREAM_TITLE',
  };
}

export function currentTrackLoaded(tl_track) {
  return {
    type: 'MOPIDY_CURRENT_TRACK_LOADED',
    tl_track,
  };
}

export function getNextTrack() {
  return {
    type: 'MOPIDY_GET_NEXT_TRACK',
  };
}

export function clearCurrentTrack() {
  return {
    type: 'CLEAR_CURRENT_TRACK',
  };
}

export function getQueue() {
  return {
    type: 'MOPIDY_GET_QUEUE',
  };
}

export function changeTrack(tlid) {
  return {
    type: 'MOPIDY_CHANGE_TRACK',
    tlid,
  };
}

/**
 * Playing assets
 * */

export function playURIs({
  uris = [],
  from = null,
  shuffle = false,
}) {
  return {
    type: 'MOPIDY_PLAY_URIS',
    uris,
    from,
    shuffle,
  };
}

export function playAlbum({ uri, from }) {
  return {
    type: 'MOPIDY_PLAY_ALBUM',
    uri,
    from,
  };
}

export function playPlaylist({ uri, shuffle = false }) {
  return {
    type: 'MOPIDY_PLAY_PLAYLIST',
    uri,
    shuffle,
  };
}

export function enqueueURIs({
  uris = [],
  from = null,
  shuffle = false,
  at_position = null,
  play_next = false,
  offset = 0,
}) {
  return {
    type: 'MOPIDY_ENQUEUE_URIS',
    uris,
    from,
    shuffle,
    at_position,
    play_next,
    offset,
  };
}

export function enqueueAlbum({
  uri,
  from,
  play_next = false,
  at_position = null,
}) {
  return {
    type: 'MOPIDY_ENQUEUE_ALBUM',
    uri,
    from,
    play_next,
    at_position,
  };
}

export function enqueuePlaylist({
  uri,
  from,
  play_next = false,
  at_position = null,
}) {
  return {
    type: 'MOPIDY_ENQUEUE_PLAYLIST',
    from,
    uri,
    play_next,
    at_position,
  };
}

export function enqueueURIsBatchDone() {
  return {
    type: 'MOPIDY_ENQUEUE_URIS_BATCH_DONE',
  };
}

/**
 * Playlist maniopulation
 * */

export function removeTracks(tlids) {
  return {
    type: 'MOPIDY_REMOVE_TRACKS',
    tlids,
  };
}

export function reorderTracklist(indexes, insert_before) {
  const range = createRange(indexes);
  if (insert_before > range.start) insert_before -= range.length;
  return {
    type: 'MOPIDY_REORDER_TRACKLIST',
    range_start: range.start,
    range_length: range.length,
    insert_before,
  };
}

export function clearTracklist() {
  return {
    type: 'MOPIDY_CLEAR_TRACKLIST',
  };
}

export function shuffleTracklist() {
  return {
    type: 'MOPIDY_SHUFFLE_TRACKLIST',
  };
}

/**
 * Asset-oriented actions
 * */

export function getUris(uris) {
  return {
    type: 'MOPIDY_GET_URIS',
    uris,
  };
}

export function getImages(uris) {
  return {
    type: 'MOPIDY_GET_IMAGES',
    uris,
  };
}

export function createPlaylist(playlist) {
  return {
    type: 'MOPIDY_CREATE_PLAYLIST',
    playlist,
  };
}

export function deletePlaylist(uri) {
  return {
    type: 'MOPIDY_DELETE_PLAYLIST',
    uri,
  };
}

export function getLibraryMoods(uri) {
  return {
    type: 'MOPIDY_GET_LIBRARY_MOODS',
    uri,
  };
}

export function getLibraryFeaturedPlaylists(uri) {
  return {
    type: 'MOPIDY_GET_LIBRARY_FEATURED_PLAYLISTS',
    uri,
  };
}

export function getLibraryPlaylists(uri) {
  return {
    type: 'MOPIDY_GET_LIBRARY_PLAYLISTS',
    uri,
  };
}

export function getPlaylistGroup(uri, options) {
  return {
    type: 'MOPIDY_GET_PLAYLIST_GROUP',
    uri,
    options,
  };
}

export function getPlaylist(uri, options) {
  return {
    type: 'MOPIDY_GET_PLAYLIST',
    uri,
    options,
  };
}

export function getPlaylists(uris, processor = null) {
  return {
    type: 'MOPIDY_GET_PLAYLISTS',
    uris,
    processor,
  };
}

export function getDirectory(uri) {
  return {
    type: 'MOPIDY_GET_DIRECTORY',
    uri,
  };
}

export function getTracks(uris, options) {
  return {
    type: 'MOPIDY_GET_TRACKS',
    uris,
    options,
  };
}

export function getTrack(uri, options) {
  return getTracks([uri], options);
}

export function getLibraryArtists(uri = null) {
  return {
    type: 'MOPIDY_GET_LIBRARY_ARTISTS',
    uri,
  };
}

export function getLibraryAlbums(uri = null) {
  return {
    type: 'MOPIDY_GET_LIBRARY_ALBUMS',
    uri,
  };
}

export function getLibraryTracks(uri = null) {
  return {
    type: 'MOPIDY_GET_LIBRARY_TRACKS',
    uri,
  };
}

export function getArtist(uri, options) {
  return {
    type: 'MOPIDY_GET_ARTIST',
    uri,
    options,
  };
}

export function getAlbum(uri, options) {
  return {
    type: 'MOPIDY_GET_ALBUM',
    uri,
    options,
  };
}

export function view_getRandomTracks(limit = 100) {
  return {
    type: 'VIEW__GET_RANDOM_TRACKS',
    limit,
  };
}

export function clearSearchResults() {
  return {
    type: 'MOPIDY_CLEAR_SEARCH_RESULTS',
  };
}

export function getSearchResults(query, providers, limit = 100) {
  return {
    type: 'MOPIDY_GET_SEARCH_RESULTS',
    query,
    providers,
    limit,
  };
}

export function getQueueHistory() {
  return {
    type: 'MOPIDY_GET_QUEUE_HISTORY',
  };
}

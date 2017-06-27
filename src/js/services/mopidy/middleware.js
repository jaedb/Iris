
import Mopidy from 'mopidy'
import * as helpers from '../../helpers'

var mopidyActions = require('./actions.js')
var uiActions = require('../ui/actions.js')
var spotifyActions = require('../spotify/actions.js')
var pusherActions = require('../pusher/actions.js')
var lastfmActions = require('../lastfm/actions.js')

const MopidyMiddleware = (function(){ 

    // container for the actual Mopidy socket
    var socket = null

    // play position timer
    var progress_interval = null
    var progress_interval_counter = 0

    // handle all manner of socket messages
    const handleMessage = (ws, store, type, data) => {

        // if debug enabled
        if (store.getState().ui.log_mopidy) console.log('Mopidy', type, data)

        switch( type ){

            case 'state:online':
                store.dispatch({ type: 'MOPIDY_CONNECTED' });
                instruct( ws, store, 'playback.getState' );
                instruct( ws, store, 'playback.getVolume' );
                instruct( ws, store, 'mixer.getMute' );
                instruct( ws, store, 'tracklist.getConsume' );
                instruct( ws, store, 'tracklist.getRandom' );
                instruct( ws, store, 'tracklist.getRepeat' );
                instruct( ws, store, 'tracklist.getTlTracks' );
                instruct( ws, store, 'playback.getCurrentTlTrack' );
                instruct( ws, store, 'playback.getTimePosition' );
                instruct( ws, store, 'getUriSchemes' );

                // every 1000s update our play position (when playing)
                progress_interval = setInterval(() => {
                    if (store.getState().mopidy.play_state == 'playing'){

                        // every 10s get real position from server
                        if( progress_interval_counter % 10 == 0 ){
                            store.dispatch(mopidyActions.getTimePosition())

                        // otherwise we just assume to add 1000ms every 1000ms of play time
                        }else{
                            store.dispatch(mopidyActions.setTimePosition( store.getState().mopidy.time_position + 1000 ))              
                        }

                        progress_interval_counter++
                    }
                }, 1000);

                break;

            case 'state:offline':
                store.dispatch({ type: 'MOPIDY_DISCONNECTED' });

                // reset our playback interval timer
                clearInterval(progress_interval)
                progress_interval_counter = 0
                break;

            case 'event:tracklistChanged':
                instruct( ws, store, 'tracklist.getTlTracks' );
                break;

            case 'event:playbackStateChanged':
                instruct( ws, store, 'playback.getState' );
                instruct( ws, store, 'playback.getTimePosition' );
                break;

            case 'event:seeked':
                store.dispatch({ type: 'MOPIDY_TIMEPOSITION', data: data.time_position });
                break;

            case 'event:trackPlaybackEnded':
                instruct( ws, store, 'playback.getTimePosition' );
                break;

            case 'event:trackPlaybackStarted':
                instruct( ws, store, 'playback.getCurrentTlTrack' );
                break;

            case 'event:volumeChanged':
                store.dispatch({ type: 'MOPIDY_VOLUME', data: data.volume });
                break;

            case 'event:muteChanged':
                store.dispatch({ type: 'MOPIDY_MUTE', data: data.mute });
                break;

            case 'event:optionsChanged':
                instruct( ws, store, 'tracklist.getConsume' );
                instruct( ws, store, 'tracklist.getRandom' );
                instruct( ws, store, 'tracklist.getRepeat' );
                break;

            default:
                //console.log( 'MopidyService: Unhandled message', type, message );
        }
    }


    /**
     * Call something with Mopidy
     *
     * Sends request to Mopidy server, and updates our local storage on return
     * @param object ws = Mopidy class that wraps a Websocket
     * @param string model = which Mopidy model (playback, tracklist, etc)
     * @param string property = TlTracks, Consume, etc
     * @param string value (optional) = value of the property to pass
     * @return promise
     **/
    const instruct = (ws, store, call, value = {}) => {

        if( !store.getState().mopidy.connected ) return false

        var callParts = call.split('.');
        var model = callParts[0];
        var method = callParts[1];

        return new Promise( (resolve, reject) => {

            if( model in ws ){
                if( method in ws[model] ){
                    var mopidyObject = ws[model][method]
                    var property = method;       
                }else{                
                    var mopidyObject = ws[model]
                    var property = model;   
                }
            }else{
                var error = {
                    message: 'Call to an invalid object. Check you are calling a valid Mopidy object.',
                    call: call,
                    value: value
                }
                console.error(error)
                reject(error)
            }

            property = property.replace('get','');
            property = property.replace('set','');

            var loader_key = helpers.generateGuid()
            store.dispatch(uiActions.startLoading(loader_key, 'mopidy_'+property))

            mopidyObject( value )
                .then(
                    response => {
                        store.dispatch(uiActions.stopLoading(loader_key))
                        store.dispatch({ type: 'MOPIDY_'+property.toUpperCase(), call: call, data: response })
                        resolve(response)
                    },
                    error => {
                        store.dispatch(uiActions.stopLoading(loader_key))
                        console.error(error)
                        reject(error)
                    }
                );
        });
    }

    /**
     * Middleware
     *
     * This behaves like an action interceptor. We listen for specific actions
     * and handle special functionality. If the action is not in our switch, then
     * it just proceeds to the next middleware, or default functionality
     **/
    return store => next => action => {

        switch(action.type) {

            case 'MOPIDY_CONNECT':

                if(socket != null) socket.close()
                store.dispatch({ type: 'MOPIDY_CONNECTING' })
                var state = store.getState()

                socket = new Mopidy({
                    webSocketUrl: 'ws'+(window.location.protocol === 'https:' ? 's' : '')+'://'+state.mopidy.host+':'+state.mopidy.port+'/mopidy/ws/',
                    callingConvention: 'by-position-or-by-name'
                })

                socket.on( (type, data) => handleMessage( socket, store, type, data ) )
                break

            case 'MOPIDY_DISCONNECT':
                if(socket != null) socket.close()
                socket = null
                store.dispatch({ type: 'MOPIDY_DISCONNECTED' })
                break

            // send an instruction to the websocket
            case 'MOPIDY_INSTRUCT':
                instruct( socket, store, action.call, action.value )
                break

            case 'MOPIDY_DEBUG':
                instruct( socket, store, action.call, action.value )
                    .then( response => {
                        store.dispatch({ type: 'DEBUG', response: response })
                    })
                break

            case 'MOPIDY_URISCHEMES':
                var uri_schemes = action.data
                var remove = ['http','https','mms','rtmp','rtmps','rtsp','sc','spotify']

                // remove all our ignored types
                for( var i = 0; i < remove.length; i++ ){
                    var index = uri_schemes.indexOf(remove[i])
                    if( index > -1 ) uri_schemes.splice(index, 1);
                }

                // append with ':' to make them a mopidy URI
                for( var i = 0; i < uri_schemes.length; i++ ){
                    uri_schemes[i] = uri_schemes[i] +':'
                }

                store.dispatch({ type: 'MOPIDY_URISCHEMES_FILTERED', data: uri_schemes });
                break


            /**
             * General playback
             **/

            case 'MOPIDY_NEXT':
                var data = {
                    type: 'browser_notification',
                    title: 'Track skipped',
                    body: store.getState().pusher.username +' skipped this track',
                    icon: (store.getState().ui.current_track ? helpers.getTrackIcon( store.getState().ui.current_track ) : false)
                }
                store.dispatch( pusherActions.deliverBroadcast(data) )
                break

            case 'MOPIDY_STOP':
                var data = {
                    type: 'browser_notification',
                    title: 'Playback stopped',
                    body: store.getState().pusher.username +' stopped playback',
                    icon: (store.getState().ui.current_track ? helpers.getTrackIcon( store.getState().ui.current_track ) : false)
                }
                store.dispatch( pusherActions.deliverBroadcast(data) )
                break

            case 'MOPIDY_PLAY_PLAYLIST':

                // playlist already in index
                if (store.getState().ui.playlists.hasOwnProperty(action.uri)){
                    
                    // make sure we didn't get this playlist from Mopidy-Spotify
                    // if we did, we'd have a cached version on server so no need to fetch
                    if (!store.getState().ui.playlists[action.uri].is_mopidy){
                        store.dispatch(uiActions.startProcess('MOPIDY_ENQUEUE_URIS', 'Fetching tracks'))
                        store.dispatch(spotifyActions.getAllPlaylistTracks(action.uri))
                        break
                    }

                // it's a spotify playlist that we haven't loaded
                // we need to fetch via HTTP API to avoid timeout
                } else if (helpers.uriSource(action.uri) == 'spotify'){
                    store.dispatch(uiActions.startProcess('MOPIDY_ENQUEUE_URIS', 'Fetching tracks'))
                    store.dispatch(spotifyActions.getAllPlaylistTracks(action.uri))
                    break
                }

                // fetch the playlist tracks via backend
                // add each track by URI
                instruct(socket, store, 'playlists.lookup', {uri: action.uri})
                .then( response => {
                    if (typeof(response.tracks) === 'undefined'){
                        store.dispatch(uiActions.createNotification('Failed to load playlist tracks','bad'))
                    } else {
                        var tracks_uris = helpers.asURIs(response.tracks)
                        store.dispatch(mopidyActions.playURIs(tracks_uris, action.uri))
                    }
                })
                break

            case 'SPOTIFY_ALL_PLAYLIST_TRACKS_LOADED_FOR_PLAYING':
                var uris = []
                for (var i = 0; i < action.tracks.length; i++){
                    uris.push(action.tracks[i].track.uri)
                }
                store.dispatch(mopidyActions.playURIs(uris, action.uri))
                break

            case 'MOPIDY_ENQUEUE_URIS':

                // split into batches
                var uris = Object.assign([], action.uris)
                var batches = []
                var batch_size = 5
                while (uris.length > 0){
                    batches.push({
                        uris: uris.splice(0,batch_size),
                        at_position: action.at_position,
                        next: action.next,
                        offset: action.offset + (batch_size * batches.length),
                        from_uri: action.from_uri
                    })
                }

                // pass this modified action to the reducer (and other middleware)
                action.batches = batches
                next(action)

                // start our processor
                store.dispatch(mopidyActions.enqueueUrisProcessor())
                break

            case 'MOPIDY_ENQUEUE_URIS_PROCESSOR':

                // make sure we have some uris in the queue
                if (store.getState().mopidy.enqueue_uris_batches && store.getState().mopidy.enqueue_uris_batches.length > 0){

                    var batches = store.getState().mopidy.enqueue_uris_batches
                    var batch = batches[0]
                    var total_uris = 0
                    for (var i = 0; i < batches.length; i++){
                        total_uris += batches[i].uris.length
                    }
                    store.dispatch(uiActions.startProcess('MOPIDY_ENQUEUE_URIS', 'Adding '+total_uris+' URI(s)'))

                // no batches means we're done here
                } else {
                    store.dispatch(uiActions.stopProcess('MOPIDY_ENQUEUE_URIS'))
                    break
                }

                var current_track = store.getState().ui.current_track
                var current_tracklist = store.getState().ui.current_tracklist
                var current_track_index = -1

                if (typeof(current_track) !== 'undefined'){
                    for( var i = 0; i < current_tracklist.length; i++ ){
                        if( current_tracklist[i].tlid == current_track.tlid ){
                            current_track_index = i
                            break
                        }
                    }
                }

                var params = {uris: batch.uris}

                // Play this batch next
                if (batch.next){

                    // Make sure we're playing something first
                    if (current_track_index > -1){
                        params.at_position = current_track_index + batch.offset + 1

                    // Default to top of queue if we're not playing
                    } else {
                        params.at_position = 0 + batch.offset
                    }

                // A specific position has been defined
                // NOTE: This is likely to be wrong as the original action is unaware of batches or other client requests
                } else if (batch.at_position){
                    params.at_position = batch.at_position + batch.offset
                }

                instruct(socket, store, 'tracklist.add', params)
                    .then( response => {

                        // add metadata to queue
                        var tlids = []
                        for (var i = 0; i < response.length; i++){
                            tlids.push(response[i].tlid)
                        }
                        store.dispatch(pusherActions.addQueueMetadata(tlids, batch.from_uri))

                        // Re-run the batch checker in 100ms. This allows a small window for other
                        // server requests before our next batch. It's a little crude but it means the server isn't
                        // locked until we're completely done.
                        setTimeout(
                            function(){ 
                                store.dispatch(mopidyActions.enqueueURIsBatchDone())
                                store.dispatch(mopidyActions.enqueueUrisProcessor())
                            }, 
                            100
                        )
                    })

                break

            case 'CANCEL_PROCESS':
                if (action.key == 'MOPIDY_ENQUEUE_URIS'){
                    store.dispatch(mopidyActions.enqueueURIsCancel())
                }
                next(action)
                break

            case 'MOPIDY_PLAY_URIS':

                // stop the radio
                if (store.getState().ui.radio && store.getState().ui.radio.enabled){
                    store.dispatch( pusherActions.stopRadio() )
                }

                var first_uri = action.uris[0]

                // add our first track
                instruct(socket, store, 'tracklist.add', { uri: first_uri, at_position: 0 })
                    .then(response => {

                        // play it (only if we got a successful lookup)
                        if (response.length > 0){
                            store.dispatch(mopidyActions.changeTrack(response[0].tlid));

                            var tlids = []
                            for (var i = 0; i < response.length; i++){
                                tlids.push(response[i].tlid)
                            }
                            store.dispatch(pusherActions.addQueueMetadata(tlids, action.from_uri))
                        } else {
                            store.dispatch(uiActions.createNotification('Failed to add some URI(s)', 'bad'))
                            console.error('Failed to add some URI(s)', response)
                        }

                        // add the rest of our uris (if any)
                        action.uris.shift();
                        if( action.uris.length > 0 ){

                            // wait 100ms so the server can trigger track_changed etc
                            // this means our UI feels snappier as the first track shows up quickly
                            setTimeout(
                                function(){ 
                                    store.dispatch(mopidyActions.enqueueURIs(action.uris, action.from_uri, null, 1))
                                }, 
                                100
                            )
                        }
                    })
                break;

            case 'MOPIDY_REORDER_TRACKLIST':

                // add our first track
                instruct( socket, store, 'tracklist.move', { start: action.range_start, end: action.range_start + action.range_length, to_position: action.insert_before } )
                    .then( response => {
                        // TODO: when complete, send event to confirm success/failure
                    })
                break;


            /**
             * =============================================================== SEARCHING ============
             * ======================================================================================
             **/


            case 'MOPIDY_GET_TRACK_SEARCH_RESULTS':
                instruct( socket, store, 'library.search', {query: {any: [action.query]}, uris: [action.uri_scheme]})
                    .then( response => {
                        if (response.length <= 0) return
                        if (typeof(response[0].tracks) === 'undefined') return

                        var tracks = response[0].tracks.splice(0,action.limit)

                        store.dispatch({ type: 'SEARCH_RESULTS_LOADED', tracks: tracks });
                    })
                break;

            case 'MOPIDY_GET_ARTIST_SEARCH_RESULTS':
                instruct( socket, store, 'library.search', {query: {artist: [action.query]}, uris: [action.uri_scheme]})
                    .then( response => { 
                        if (response.length <= 0) return

                        var artists_uris = []
                        if (response[0].tracks){
                            for (var i = 0; i < response[0].tracks.length; i++){
                                if (response[0].tracks[i].artists){
                                    for (var j = 0; j < response[0].tracks[i].artists.length; j++){
                                        var artist = response[0].tracks[i].artists[j]
                                        if (artist.uri){
                                            artists_uris.push(artist.uri)
                                        }
                                    }
                                }
                            }
                        }

                        // TODO: limit uris at the loop, rather than post loop for performance
                        artists_uris = helpers.removeDuplicates(artists_uris).splice(0, action.limit)

                        // load each artist
                        for (var i = 0; i < artists_uris.length; i++){
                            store.dispatch(mopidyActions.getArtist(artists_uris[i]))
                        }

                        // and plug in their URIs
                        store.dispatch({ type: 'SEARCH_RESULTS_LOADED', artists_uris: artists_uris })
                    })
                break;

            case 'MOPIDY_GET_ALBUM_SEARCH_RESULTS':
                instruct( socket, store, 'library.search', {query: {album: [action.query]}, uris: [action.uri_scheme]})
                    .then( response => {
                        if (response.length <= 0) return

                        // collate all our different sources into one array
                        var albums_uris = []
                        if (response[0].tracks){
                            for (var i = 0; i < response[0].tracks.length; i++){
                                if (response[0].tracks[i].album){
                                    var album = response[0].tracks[i].album
                                    if (album.uri){
                                        albums_uris.push(album.uri)
                                    }
                                }
                            }
                        }

                        // TODO: limit uris at the loop, rather than post loop for performance
                        albums_uris = helpers.removeDuplicates(albums_uris).splice(0, action.limit)

                        // load each album
                        for (var i = 0; i < albums_uris.length; i++){
                            store.dispatch(mopidyActions.getAlbum(albums_uris[i]))
                        }

                        // and plug in their URIs
                        store.dispatch({ type: 'SEARCH_RESULTS_LOADED', albums_uris: albums_uris })
                    })
                break

            case 'MOPIDY_GET_PLAYLIST_SEARCH_RESULTS':
                instruct( socket, store, 'playlists.asList')
                    .then( response => {
                        if (response.length <= 0) return

                        var playlists_uris = []
                        for (var i = 0; i < response.length; i++){
                            var playlist = response[i]
                            if (playlist.name.includes(action.query) && action.uri_scheme == helpers.uriSource(playlist.uri)+':'){
                                playlists_uris.push(playlist.uri)
                            }
                        }

                        playlists_uris = playlists_uris.splice(0, action.limit)

                        // load each playlist
                        for (var i = 0; i < playlists_uris.length; i++){
                            store.dispatch(mopidyActions.getPlaylist(playlists_uris[i]))
                        }

                        // and plug in their URIs
                        store.dispatch({ type: 'SEARCH_RESULTS_LOADED', playlists_uris: playlists_uris })
                    })
                break


            /**
             * =============================================================== PLAYLIST(S) ==========
             * ======================================================================================
             **/

            case 'MOPIDY_GET_LIBRARY_PLAYLISTS':
                instruct( socket, store, 'playlists.asList' )
                    .then( response => {

                        // drop in our URI list
                        var playlist_uris = helpers.asURIs(response)

                        store.dispatch({ type: 'LIBRARY_PLAYLISTS_LOADED', uris: playlist_uris });

                        // get the full playlist objects
                        for (var i = 0; i < response.length; i++ ){
                            instruct( socket, store, 'playlists.lookup', { uri: response[i].uri })
                                .then( response => {
                                    var source = helpers.uriSource(response.uri)
                                    var playlist = Object.assign(
                                        {},
                                        {
                                            type: 'playlist',
                                            name: response.name,
                                            uri: response.uri,
                                            source: source,
                                            is_mopidy: true,
                                            last_modified: response.last_modified,
                                            tracks_total: ( response.tracks ? response.tracks.length : 0 )
                                        }
                                    )

                                    store.dispatch({ 
                                        type: 'PLAYLIST_LOADED', 
                                        key: playlist.uri,
                                        playlist: playlist 
                                    })
                                })
                        }
                    })
                break;

            case 'MOPIDY_GET_PLAYLIST':
                instruct( socket, store, 'playlists.lookup', action.data )
                    .then( response => {
                        var playlist = Object.assign(
                            {},
                            response,
                            {
                                type: 'playlist',
                                is_mopidy: true,
                                tracks: ( response.tracks ? response.tracks : [] ),
                                tracks_total: ( response.tracks ? response.tracks.length : [] )
                            }
                        )

                        // tracks? get the full track objects
                        if( playlist.tracks.length > 0 ) store.dispatch({
                            type: 'MOPIDY_RESOLVE_PLAYLIST_TRACKS', 
                            tracks: playlist.tracks, 
                            key: response.uri
                        })

                        store.dispatch({ 
                            type: 'PLAYLIST_LOADED',
                            key: playlist.uri,
                            playlist: playlist
                        })
                    })
                break;

            case 'MOPIDY_RESOLVE_PLAYLIST_TRACKS':
                var tracks = Object.assign([], action.tracks)
                var uris = helpers.asURIs(tracks)

                instruct( socket, store, 'library.lookup', { uris: uris } )
                    .then( response => {
                        for(var uri in response){
                            if (response.hasOwnProperty(uri)) {

                                var track = response[uri][0]
                                if( track ){
                                    
                                    // find the track reference, and drop in the full track data
                                    function getByURI( trackReference ){
                                        return track.uri == trackReference.uri
                                    }
                                    var trackReferences = tracks.filter(getByURI);
                                    
                                    // there could be multiple instances of this track, so accommodate this
                                    for( var j = 0; j < trackReferences.length; j++){
                                        var key = tracks.indexOf( trackReferences[j] );
                                        tracks[ key ] = track;
                                    }
                                }
                            }
                        }

                        store.dispatch({ 
                            type: 'PLAYLIST_TRACKS', 
                            tracks: tracks, 
                            key: action.key 
                        })
                    })
                break

            case 'MOPIDY_ADD_PLAYLIST_TRACKS':
                
                instruct( socket, store, 'playlists.lookup', { uri: action.key })
                    .then( response => {
                        var tracks = [];             
                        for( var i = 0; i < action.tracks_uris.length; i++ ){
                            tracks.push({
                                __model__: "Track",
                                uri: action.tracks_uris[i]
                            });
                        }

                        var playlist = Object.assign({}, response)
                        if( playlist.tracks ){
                            playlist.tracks = [...playlist.tracks, ...tracks]
                        }else{
                            playlist.tracks = tracks
                        }

                        instruct( socket, store, 'playlists.save', { playlist: playlist } )
                            .then( response => {
                                store.dispatch({ 
                                    type: 'PLAYLIST_TRACKS_ADDED', 
                                    key: action.key, 
                                    tracks_uris: action.tracks_uris
                                });
                            })
                    });
                break

            case 'MOPIDY_REMOVE_PLAYLIST_TRACKS':

                // reverse order our indexes (otherwise removing from top will affect the keys following)           
                function descending(a,b){
                    return b-a;
                }
                var indexes = Object.assign([], action.tracks_indexes)
                indexes.sort(descending);
                
                instruct( socket, store, 'playlists.lookup', { uri: action.key })
                    .then( response => {
                        var playlist = Object.assign({}, response)
                        for( var i = 0; i < indexes.length; i++ ){
                            playlist.tracks.splice(indexes[i], 1);
                        }
                        instruct( socket, store, 'playlists.save', { playlist: playlist } )
                            .then( response => {
                                store.dispatch({
                                    type: 'PLAYLIST_TRACKS_REMOVED', 
                                    key: action.key, 
                                    tracks_indexes: action.tracks_indexes
                                });
                            })
                    });
                break

            case 'MOPIDY_SAVE_PLAYLIST':                
                instruct( socket, store, 'playlists.lookup', { uri: action.key })
                    .then( response => {
                        var playlist = Object.assign({}, response, { name: action.name })
                        instruct( socket, store, 'playlists.save', { playlist: playlist } )
                            .then( response => {
                                store.dispatch({ 
                                    type: 'PLAYLIST_UPDATED', 
                                    key: action.key, 
                                    playlist: playlist
                                })
                            })
                    });
                break

            case 'MOPIDY_REORDER_PLAYLIST_TRACKS':                
                instruct( socket, store, 'playlists.lookup', { uri: action.key })
                    .then( response => {

                        var playlist = Object.assign({}, response)
                        var tracks = Object.assign([], playlist.tracks)
                        var tracks_to_move = []

                        // calculate destination index: if dragging down, accommodate the offset created by the tracks we're moving
                        var range_start = action.range_start
                        var range_length = action.range_length
                        var insert_before = action.insert_before
                        if( insert_before > range_start ) insert_before = insert_before - range_length

                        // collate our tracks to be moved
                        for( var i = 0; i < range_length; i++ ){

                            // add to FRONT: we work backwards to avoid screwing up our indexes
                            tracks_to_move.unshift( tracks[range_start + i] )
                        }

                        // remove tracks from their old location
                        tracks.splice( range_start, range_length )

                        // now plug them back in, in their new location
                        for( var i = 0; i < tracks_to_move.length; i++ ){
                            tracks.splice( insert_before, 0, tracks_to_move[i] )
                        }

                        // update playlist
                        playlist = Object.assign({}, playlist, { tracks: tracks })
                        instruct( socket, store, 'playlists.save', { playlist: playlist } )
                            .then( response => {

                                store.dispatch({ 
                                    type: 'MOPIDY_RESOLVE_PLAYLIST_TRACKS', 
                                    tracks: playlist.tracks, 
                                    key: playlist.uri
                                })
                            })
                    });
                break

            case 'MOPIDY_CREATE_PLAYLIST':
                instruct( socket, store, 'playlists.create', { name: action.name, uri_scheme: action.scheme })
                    .then( response => {

                        // re-load our global playlists
                        //store.dispatch({ type: 'MOPIDY_GET_PLAYLISTS' });
                    });
                break

            case 'MOPIDY_DELETE_PLAYLIST':
                instruct( socket, store, 'playlists.delete', { uri: action.key })
                    .then( response => {

                        // re-load our global playlists
                        // store.dispatch({ type: 'MOPIDY_PLAYLISTS' });
                    });           
                break
                

            /**
             * =============================================================== ALBUM(S) =============
             * ======================================================================================
             **/

            case 'MOPIDY_GET_LIBRARY_ALBUMS':
                instruct( socket, store, 'library.browse', { uri: 'local:directory?type=album' } )
                    .then( response => {
                        if (response.length <= 0) return

                        var uris = helpers.asURIs(response)

                        store.dispatch({ 
                            type: 'MOPIDY_GET_ALBUMS',
                            uris: uris.slice(0,50)
                        });

                        store.dispatch({ 
                            type: 'LOCAL_ALBUMS_LOADED', 
                            uris: uris
                        });
                    })
                break;

            case 'MOPIDY_GET_ALBUMS':
                instruct( socket, store, 'library.lookup', { uris: action.uris } )
                    .then( response => {
                        if (response.length <= 0) return

                        var albums = []

                        for (var uri in response){
                            if (response.hasOwnProperty(uri) && response[uri].length > 0 && response[uri][0] && response[uri][0].album ){
                                var album = Object.assign(
                                    {},
                                    {
                                        is_mopidy: true,
                                        artists: response[uri][0].artists,
                                        tracks: response[uri],
                                        tracks_total: response[uri].length
                                    },
                                    response[uri][0].album
                                )

                                albums.push(album)
                            }
                        }

                        store.dispatch({ 
                            type: 'ALBUMS_LOADED',
                            albums: albums
                        });
                    })
                break;

            case 'MOPIDY_GET_ALBUM':
                instruct( socket, store, 'library.lookup', action.data )
                    .then( response => {
                        if (response.length <= 0) return
                            
                        var album = Object.assign(
                            {},
                            { images: [] },
                            response[0].album,
                            {
                                is_mopidy: true,
                                artists: response[0].artists,
                                tracks: response,
                                tracks_total: response.length
                            }
                        )
                        
                        var uris = [];
                        for( var i = 0; i < album.tracks.length; i++ ){
                            uris.push( album.tracks[i].uri );
                        }

                         // load artwork from LastFM
                        if( album.images.length <= 0 ){

                            var mbid = helpers.getFromUri('mbid',album.uri)
                            if( mbid ){
                                store.dispatch( lastfmActions.getAlbum( false, false, mbid ) )
                            }else{
                                store.dispatch( lastfmActions.getAlbum( album.artists[0].name, album.name ) )
                            }
                        }

                        instruct( socket, store, 'library.lookup', { uris: uris } )
                            .then( response => {

                                for(var uri in response){
                                    if (response.hasOwnProperty(uri)) {

                                        var track = response[uri][0];
                                        
                                        // find the track reference, and drop in the full track data
                                        function getByURI( trackReference ){
                                            return track.uri == trackReference.uri
                                        }
                                        var trackReferences = album.tracks.filter(getByURI);
                                        
                                        // there could be multiple instances of this track, so accommodate this
                                        for( var j = 0; j < trackReferences.length; j++){
                                            var key = album.tracks.indexOf( trackReferences[j] );
                                            album.tracks[ key ] = track;
                                        }
                                    }
                                }

                                store.dispatch({ 
                                    type: 'ALBUM_LOADED', 
                                    key: album.uri,
                                    album: album 
                                });
                            })
                    })
                break;
                

            /**
             * =============================================================== ARTIST(S) ============
             * ======================================================================================
             **/

            case 'MOPIDY_GET_LIBRARY_ARTISTS':
                store.dispatch({ type: 'LOCAL_ARTISTS_LOADED', data: false });
                instruct( socket, store, 'library.browse', { uri: 'local:directory?type=artist' } )
                    .then( response => {                    
                        store.dispatch({ 
                            type: 'ARTISTS_LOADED', 
                            artists: response
                        });               
                        store.dispatch({ 
                            type: 'LOCAL_ARTISTS_LOADED', 
                            uris: helpers.asURIs(response)
                        });
                    })
                break;

            case 'MOPIDY_GET_ARTIST':
                instruct( socket, store, 'library.lookup', action.data )
                    .then( response => {
                        if (response.length <= 0) return
                                          
                        var albums = []
                        for( var i = 0; i < response.length; i++ ){
                            var album = response[i].album;
                            if (album){
                                function getByURI( albumToCheck ){
                                    return album.uri == albumToCheck.uri
                                }
                                var existingAlbum = albums.find(getByURI);
                                if( !existingAlbum ){
                                    albums.push(album)
                                }
                            }
                        }
                        if (albums){
                            store.dispatch({
                                type: 'ALBUMS_LOADED',
                                albums: albums
                            })
                        }

                        var artist = Object.assign(
                            {},
                            (response ? response[0].artists[0] : {}),
                            {
                                is_mopidy: true,
                                albums_uris: helpers.asURIs(albums),
                                tracks: response.slice(0,10)
                            }
                        )
                        
                        store.dispatch({ 
                            type: 'ARTIST_LOADED',
                            key: artist.uri,
                            artist: artist 
                        })

                        // load artwork from LastFM
                        if( !artist.images || artist.images.length <= 0 ){
                            if( artist.musicbrainz_id ){
                                store.dispatch( lastfmActions.getArtist( artist.uri, false, artist.musicbrainz_id ) )
                            }else{
                                store.dispatch( lastfmActions.getArtist( artist.uri, artist.name.replace('&','and') ) )
                            }
                        }
                    })
                break;
                

            /**
             * =============================================================== TRACKS ================
             * ======================================================================================
             **/

            case 'MOPIDY_CURRENTTLTRACK':
                if (action.data && action.data.track){

                    // Fire off our universal track index loader
                    store.dispatch({ type: 'TRACK_LOADED', key: action.data.track.uri, track: action.data.track })

                    // When current track is Spotify track, go get the full object
                    // This is because Mopidy doesn't give us full artist/album objects, without artwork
                    if (action.data.track.uri.substring(0,14) == 'spotify:track:'){
                        store.dispatch( spotifyActions.getTrack( action.data.track.uri ) )
                    }
                }

                next(action)
                break
                

            /**
             * =============================================================== LOCAL ================
             * ======================================================================================
             **/

            case 'MOPIDY_GET_DIRECTORY':
                store.dispatch({ type: 'MOPIDY_DIRECTORY_LOADED', data: false })
                instruct( socket, store, 'library.browse', action.data )
                    .then( response => {                    
                        store.dispatch({ 
                            type: 'MOPIDY_DIRECTORY_LOADED',
                            data: response
                        });
                    })
                break

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action)
        }
    }

})();

export default MopidyMiddleware
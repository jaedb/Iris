
import Mopidy from 'mopidy'
import * as helpers from '../../helpers'

var mopidyActions = require('./actions.js')
var lastfmActions = require('../lastfm/actions.js')

const MopidyMiddleware = (function(){ 

    // container for the actual Mopidy socket
    var socket = null;

    // handle all manner of socket messages
    const handleMessage = (ws, store, type, data) => {
        switch( type ){

            case 'state:online':
                store.dispatch({ type: 'MOPIDY_CONNECTED' });
                instruct( ws, store, 'playback.getState' );
                instruct( ws, store, 'playback.getVolume' );
                instruct( ws, store, 'tracklist.getConsume' );
                instruct( ws, store, 'tracklist.getRandom' );
                instruct( ws, store, 'tracklist.getRepeat' );
                instruct( ws, store, 'tracklist.getTlTracks' );
                instruct( ws, store, 'playback.getCurrentTlTrack' );
                instruct( ws, store, 'playback.getTimePosition' );
                instruct( ws, store, 'getUriSchemes' );
                break;

            case 'state:offline':
                store.dispatch({ type: 'MOPIDY_DISCONNECTED' });
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
    const instruct = ( ws, store, call, value = {} ) => {

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
                    message: 'Call to an invalid object. Check your are calling a valid Mopidy endpoint.',
                    call: call,
                    value: value
                }
                console.error(error)
                reject(error)
            }

            property = property.replace('get','');
            property = property.replace('set','');

            mopidyObject( value )
                .then(
                    response => {
                        store.dispatch({ type: 'MOPIDY_'+property.toUpperCase(), call: call, data: response });
                        resolve(response);
                    },
                    error => {
                        console.error(error)
                        reject(error);
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

                if(socket != null) socket.close();
                store.dispatch({ type: 'MOPIDY_CONNECTING' });

                var state = store.getState();

                socket = new Mopidy({
                    webSocketUrl: 'ws://'+state.mopidy.host+':'+state.mopidy.port+'/mopidy/ws',
                    callingConvention: 'by-position-or-by-name'
                });

                socket.on( (type, data) => handleMessage( socket, store, type, data ) );

                break;

            case 'MOPIDY_DISCONNECT':
                if(socket != null) socket.close();
                socket = null;                
                store.dispatch({ type: 'MOPIDY_DISCONNECTED' });
                break;

            // send an instruction to the websocket
            case 'MOPIDY_INSTRUCT':
                instruct( socket, store, action.call, action.value )
                break;

            // send an instruction to the websocket
            case 'MOPIDY_DEBUG':
                instruct( socket, store, action.call, action.value )
                    .then( response => {
                        store.dispatch({ type: 'DEBUG', response: response })
                    })
                break;

            // send an instruction to the websocket
            case 'MOPIDY_NEXT':
                var icon = false
                if( store.getState().ui.current_track ){
                    icon = helpers.getTrackIcon( store.getState().ui.current_track )
                }
                store.dispatch({ 
                    type: 'PUSHER_SEND_BROADCAST',
                    action: 'broadcast',
                    ignore_self: true,
                    data: {
                        type: 'notification',
                        data: {
                            title: 'Track skipped',
                            body: store.getState().pusher.username +' skipped this track',
                            icon: icon
                        }
                    } 
                })
                break;

            case 'MOPIDY_STOP':
                var icon = false
                if( store.getState().ui.current_track ){
                    icon = helpers.getTrackIcon( store.getState().ui.current_track )
                }
                store.dispatch({ 
                    type: 'PUSHER_SEND_BROADCAST',
                    action: 'broadcast',
                    ignore_self: true,
                    data: {
                        type: 'notification',
                        data: {
                            title: 'Playback stopped',
                            body: store.getState().pusher.username +' stopped playback',
                            icon: icon
                        }
                    } 
                })
                break;

            // send an instruction to the websocket
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
                break;

            case 'MOPIDY_PLAY_URIS':

                // add our first track
                instruct( socket, store, 'tracklist.add', { uri: action.uris[0], at_position: 0 } )
                    .then( response => {

                        if( !response || response.length <= 0 ){
                            console.error('Could not add URI to tracklist', action.uris[0])
                        }else{
                            // play it
                            store.dispatch( mopidyActions.changeTrack( response[0].tlid ) );                            
                        }

                        // add the rest of our uris (if any)
                        action.uris.shift();
                        if( action.uris.length > 0 ){
                            store.dispatch( mopidyActions.enqueueTracks( action.uris, 1 ) )
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

            case 'MOPIDY_PLAYLISTS':
                instruct( socket, store, 'playlists.asList' )
                    .then( response => {
                        for( var i = 0; i < response.length; i++ ){
                            if( response[i].uri.startsWith('m3u:')) response[i] = Object.assign({}, response[i], { can_edit: true })
                        }
                        store.dispatch({ type: 'MOPIDY_PLAYLISTS_LOADED', data: response });

                        // TODO: dispatch an action to fetch extra data for each playlist
                        // this will then trigger an PLAYLIST_REFRESHED action to update a specific store item
                    })
                break;

            case 'MOPIDY_CREATE_PLAYLIST':
                instruct( socket, store, 'playlists.create', { name: action.name, uri_scheme: action.scheme })
                    .then( response => {

                        // re-load our global playlists
                        store.dispatch({ type: 'MOPIDY_PLAYLISTS' });
                    });           
                break

            case 'MOPIDY_PLAYLIST':
                store.dispatch({ type: 'MOPIDY_PLAYLIST_LOADED', data: false });
                instruct( socket, store, 'playlists.lookup', action.data )
                    .then( response => {
                        var playlist = Object.assign(
                            {},
                            {
                                images: []
                            },
                            response,
                            {
                                tracks: ( response.tracks ? response.tracks : [] ),
                                tracks_total: ( response.tracks ? response.tracks.length : 0 )
                            }
                        )

                        // tracks? get the full track objects
                        if( playlist.tracks.length > 0 ) store.dispatch({ type: 'MOPIDY_PLAYLIST_TRACKS', tracks: playlist.tracks })

                        store.dispatch({ type: 'MOPIDY_PLAYLIST_LOADED', data: playlist })
                    })
                break;

            case 'MOPIDY_PLAYLIST_TRACKS':
                var tracks = Object.assign([], action.tracks)
                var uris = [];
                for( var i = 0; i < tracks.length; i++ ){
                    uris.push( tracks[i].uri );
                }

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

                        store.dispatch({ type: 'PLAYLIST_TRACKS_LOADED', tracks: tracks })
                    })
                break

            case 'MOPIDY_ADD_PLAYLIST_TRACKS':
                
                instruct( socket, store, 'playlists.lookup', { uri: action.playlist_uri })
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
                                store.dispatch({ type: 'PLAYLIST_TRACKS_ADDED', tracks_uris: action.tracks_uris });
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
                
                instruct( socket, store, 'playlists.lookup', { uri: action.playlist_uri })
                    .then( response => {
                        var playlist = Object.assign({}, response)
                        for( var i = 0; i < indexes.length; i++ ){
                            playlist.tracks.splice(indexes[i], 1);
                        }
                        instruct( socket, store, 'playlists.save', { playlist: playlist } )
                            .then( response => {
                                store.dispatch({ type: 'PLAYLIST_TRACKS_REMOVED', tracks_indexes: action.tracks_indexes });
                            })
                    });
                break

            case 'MOPIDY_SAVE_PLAYLIST':                
                instruct( socket, store, 'playlists.lookup', { uri: action.uri })
                    .then( response => {
                        var playlist = Object.assign({}, response, { name: action.name })
                        instruct( socket, store, 'playlists.save', { playlist: playlist } )
                            .then( response => {

                                // TODO: changing the name changes the URI, so essentially invalidates our url...
                                // need to figure out how to handle this

                                store.dispatch({ type: 'PLAYLIST_UPDATED', playlist: playlist });
                            })
                    });
                break

            case 'MOPIDY_REORDER_PLAYLIST_TRACKS':                
                instruct( socket, store, 'playlists.lookup', { uri: action.uri })
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

                                // and now re-render our full track references
                                store.dispatch({ type: 'MOPIDY_PLAYLIST_TRACKS', tracks: playlist.tracks })
                            })
                    });
                break

            case 'MOPIDY_DELETE_PLAYLIST':
                instruct( socket, store, 'playlists.delete', { uri: action.uri })
                    .then( response => {

                        // re-load our global playlists
                        store.dispatch({ type: 'MOPIDY_PLAYLISTS' });
                    });           
                break

            case 'MOPIDY_ALBUM':
                //store.dispatch({ type: 'MOPIDY_ALBUM_LOADED', data: false });
                instruct( socket, store, 'library.lookup', action.data )
                    .then( response => {
                        var album = Object.assign(
                            {},
                            { images: [] },
                            response[0].album,
                            {
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

                                store.dispatch({ type: 'MOPIDY_ALBUM_LOADED', data: album });
                            })
                    })
                break;

            case 'MOPIDY_ARTIST':
                store.dispatch({ type: 'MOPIDY_ARTIST_LOADED', data: false });
                instruct( socket, store, 'library.lookup', action.data )
                    .then( response => {
                        var artist = response[0].artists[0];
                        if( !artist.images ) artist.images = [];
                        if( !artist.albums ) artist.albums = [];
                        artist.tracks = response.slice(0,10);
                        
                        for( var i = 0; i < response.length; i++ ){
                            var album = response[i].album;

                            function getByURI( albumToCheck ){
                                return album.uri == albumToCheck.uri
                            }
                            var existingAlbum = artist.albums.find(getByURI);
                            if( !existingAlbum ){
                                artist.albums.push(album)
                            }
                        }

                        // load artwork from LastFM
                        if( artist.images.length <= 0 ){
                            if( artist.musicbrainz_id ){
                                store.dispatch( lastfmActions.getArtist( false, artist.musicbrainz_id ) )
                            }else{
                                store.dispatch( lastfmActions.getArtist( artist.name ) )
                            }
                        }
                        
                        store.dispatch({ type: 'MOPIDY_ARTIST_LOADED', data: artist });
                    })
                break;

            case 'MOPIDY_DIRECTORY':
                store.dispatch({ type: 'MOPIDY_DIRECTORY_LOADED', data: false });
                instruct( socket, store, 'library.browse', action.data )
                    .then( response => {                    
                        store.dispatch({ type: 'MOPIDY_DIRECTORY_LOADED', data: response });
                    })
                break;

            case 'MOPIDY_ARTISTS':
                store.dispatch({ type: 'MOPIDY_ARTISTS_LOADED', data: false });
                instruct( socket, store, 'library.browse', { uri: 'local:directory?type=artist' } )
                    .then( response => {                    
                        store.dispatch({ type: 'MOPIDY_ARTISTS_LOADED', data: response });
                    })
                break;

            case 'MOPIDY_ALBUMS':
                store.dispatch({ type: 'MOPIDY_ALBUMS_LOADED', data: false });
                instruct( socket, store, 'library.browse', { uri: 'local:directory?type=album' } )
                    .then( response => {                     
                        store.dispatch({ type: 'MOPIDY_ALBUMS_LOADED', data: response });
                    })
                break;

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default MopidyMiddleware
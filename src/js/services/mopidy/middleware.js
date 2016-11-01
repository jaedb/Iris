
import Mopidy from 'mopidy'
var actions = require('./actions.js')

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
                break;

            case 'state:offline':
                store.dispatch({ type: 'MOPIDY_DISCONNECTED' });
                break;

            case 'event:tracklistChanged':
                instruct( ws, store, 'tracklist.getTlTracks' );
                break;

            case 'event:playbackStateChanged':
                instruct( ws, store, 'playback.getState' );
                break;

            //case 'event:trackPlaybackEnded':
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

        var callParts = call.split('.');
        var model = callParts[0];
        var method = callParts[1];
        var property = method;
        property = property.replace('get','');
        property = property.replace('set','');

        return new Promise( (resolve, reject) => {
            ws[model][method]( value )
                .then(
                    response => {
                        store.dispatch({ type: 'MOPIDY_'+property.toUpperCase(), model: model, data: response });
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

            case 'MOPIDY_PLAY_TRACKS':

                // add our first track
                instruct( socket, store, 'tracklist.add', { uri: action.uris[0], at_position: 0 } )
                    .then( response => {

                        // play it
                        store.dispatch( actions.changeTrack( response[0].tlid ) );

                        // TODO: perhaps force update of currentTlTrack before we proceed?
                        // this will make the UI feel snappier...

                        // add the rest of our uris (if any)
                        action.uris.shift();
                        if( action.uris.length > 0 ){
                            store.dispatch( actions.enqueueTracks( action.uris, 1 ) )
                        }
                    })
                break;

            case 'MOPIDY_PLAYLISTS':
                instruct( socket, store, 'playlists.asList' )
                    .then( response => {
                        store.dispatch({ type: 'MOPIDY_PLAYLISTS_LOADED', data: response });
                    })
                break;

            case 'MOPIDY_PLAYLIST':
                store.dispatch({ type: 'MOPIDY_PLAYLIST_LOADED', data: false });
                instruct( socket, store, 'playlists.lookup', action.data )
                    .then( response => {
                        var playlist = response;
                        playlist.tracks = {
                            items: response.tracks,
                            total: response.tracks.length
                        }
                        
                        var uris = [];
                        for( var i = 0; i < playlist.tracks.items.length; i++ ){
                            uris.push( playlist.tracks.items[i].uri );
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
                                        var trackReferences = playlist.tracks.items.filter(getByURI);
                                        
                                        // there could be multiple instances of this track, so accommodate this
                                        for( var j = 0; j < trackReferences.length; j++){
                                            var key = playlist.tracks.items.indexOf( trackReferences[j] );
                                            playlist.tracks.items[ key ] = track;
                                        }
                                    }
                                }

                                store.dispatch({ type: 'MOPIDY_PLAYLIST_LOADED', data: playlist });
                            })
                    })
                break;

            case 'MOPIDY_ALBUM':
                store.dispatch({ type: 'MOPIDY_ALBUM_LOADED', data: false });
                instruct( socket, store, 'library.lookup', action.data )
                    .then( response => {
                        var album = response[0].album;
                        album.artists = response[0].artists;
                        album.tracks = {
                            items: response,
                            total: response.length
                        }
                        
                        var uris = [];
                        for( var i = 0; i < album.tracks.items.length; i++ ){
                            uris.push( album.tracks.items[i].uri );
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
                                        var trackReferences = album.tracks.items.filter(getByURI);
                                        
                                        // there could be multiple instances of this track, so accommodate this
                                        for( var j = 0; j < trackReferences.length; j++){
                                            var key = album.tracks.items.indexOf( trackReferences[j] );
                                            album.tracks.items[ key ] = track;
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
                        artist.images = [];
                        artist.albums = [];
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
                        
                        store.dispatch({ type: 'MOPIDY_ARTIST_LOADED', data: artist });
                    })
                break;

            case 'MOPIDY_ARTISTS':
                store.dispatch({ type: 'MOPIDY_ARTISTS_LOADED', data: false });
                instruct( socket, store, 'library.browse', { uri: 'local:directory?type=artist' } )
                    .then( response => {
                        var artists = response;
                        
                        store.dispatch({ type: 'MOPIDY_ARTISTS_LOADED', data: artists });
                    })
                break;

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default MopidyMiddleware

export default function reducer(mopidy = {}, action){
    switch (action.type) {

        case 'MOPIDY_CONNECTED':
            return Object.assign({}, mopidy, { connected: true, connecting: false });

        case 'MOPIDY_DISCONNECTED':
            return Object.assign({}, mopidy, { connected: false, connecting: false });

        case 'MOPIDY_SET_CONFIG':
            return Object.assign({}, mopidy, {
                host: action.host, 
                port: action.port
            });

        case 'MOPIDY_CHANGE_TRACK':
            return Object.assign({}, mopidy, {
                tlid: action.tlid
            });

        case 'MOPIDY_PLAYLISTS_LOADED':
            if( !action.data ) return mopidy;
            return Object.assign({}, mopidy, { playlists: action.data });


        /**
         * Websocket-initiated actions
         **/
        case 'MOPIDY_STATE':
            return Object.assign({}, mopidy, {
                state: action.data 
            });

        case 'MOPIDY_CONSUME':
            return Object.assign({}, mopidy, {
                consume: action.data 
            });

        case 'MOPIDY_RANDOM':
            return Object.assign({}, mopidy, {
                random: action.data 
            });

        case 'MOPIDY_REPEAT':
            return Object.assign({}, mopidy, {
                repeat: action.data 
            });

        case 'MOPIDY_TLTRACKS':
            return Object.assign({}, mopidy, {
                tracks: action.data 
            });

        case 'MOPIDY_CURRENTTLTRACK':
            if( !action.data ) return mopidy;
            
            var tracks = [];
            Object.assign(tracks, mopidy.tracks);

            for( var i = 0; i < tracks.length; i++ ){
                Object.assign(tracks[i].track, { playing: ( tracks[i].tlid == action.data.tlid ) });
            }

            return Object.assign({}, mopidy, {
                current_tltrack: action.data,
                tracks: tracks
            });

        case 'MOPIDY_VOLUME':
            return Object.assign({}, mopidy, {
                volume: action.data   
            });

        case 'MOPIDY_TIMEPOSITION':
            return Object.assign({}, mopidy, {
                time_position: action.data
            });

        case 'MOPIDY_DIRECTORY_LOADED':
            return Object.assign({}, mopidy, {
                directory: action.data   
            });

        case 'MOPIDY_PLAYLIST_LOADED':
            return Object.assign({}, mopidy, {
                playlist: action.data
            });

        case 'MOPIDY_ALBUM_LOADED':
            return Object.assign({}, mopidy, {
                album: action.data
            });

        case 'MOPIDY_ARTIST_LOADED':
            return Object.assign({}, mopidy, {
                artist: action.data
            });

        case 'MOPIDY_ARTISTS_LOADED':
            return Object.assign({}, mopidy, {
                artists: action.data
            });

        case 'MOPIDY_ALBUMS_LOADED':
            return Object.assign({}, mopidy, {
                albums: action.data
            });

        default:
            return mopidy
    }
}




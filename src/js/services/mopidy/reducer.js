
export default function reducer(mopidy = {}, action){
    switch (action.type) {

        case 'MOPIDY_CONNECT':
        case 'MOPIDY_CONNECTING':
            return Object.assign({}, mopidy, { connected: false, connecting: true });

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

        case 'MOPIDY_URISCHEMES_FILTERED':
            return Object.assign({}, mopidy, {
                uri_schemes: action.data
            });


        /**
         * State-oriented actions
         **/
        case 'MOPIDY_STATE':
            return Object.assign({}, mopidy, {
                play_state: action.data 
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

        case 'MOPIDY_VOLUME':
            return Object.assign({}, mopidy, {
                volume: action.data   
            });

        case 'MOPIDY_TIMEPOSITION':
            return Object.assign({}, mopidy, {
                time_position: action.data
            });


        /**
         * Asset-oriented actions
         **/

        case 'MOPIDY_DIRECTORY_LOADED':
            return Object.assign({}, mopidy, {
                directory: action.data   
            });

        case 'MOPIDY_PLAYLIST_LOADED':
            return Object.assign({}, mopidy, {
                playlist: action.data
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





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
                port: action.port,
                ssl: action.ssl
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

        case 'MOPIDY_MUTE':
            return Object.assign({}, mopidy, {
                mute: action.data
            });

        case 'MOPIDY_TIMEPOSITION':
            return Object.assign({}, mopidy, {
                time_position: action.data
            });

        case 'MOPIDY_HISTORY':
            var history = []
            for (var i = 0; i < action.data.length; i++){
                history.push(Object.assign(
                    {},
                    action.data[i][1],
                    {
                        played_at: action.data[i][0],
                        type: 'history'
                    }
                ))
            }
            return Object.assign({}, mopidy, {
                queue_history: history
            });


        /**
         * Asset-oriented actions
         **/

        case 'MOPIDY_DIRECTORY_LOADED':
            return Object.assign({}, mopidy, {
                directory: action.data   
            });

        case 'MOPIDY_ENQUEUE_URIS':
            if (mopidy.enqueue_uris_batches){
                var batches = [...mopidy.enqueue_uris_batches, ...action.batches]
            } else {
                var batches = Object.assign([],action.batches)
            }
            return Object.assign({}, mopidy, {
                enqueue_uris_batches: batches  
            });

        case 'MOPIDY_ENQUEUE_URIS_PROCESSOR_CANCEL':
            return Object.assign({}, mopidy, {
                enqueue_uris_batches: []  
            });

        case 'MOPIDY_ENQUEUE_URIS_BATCH_DONE':
            if (!mopidy.enqueue_uris_batches || mopidy.enqueue_uris_batches.length <= 0){

                // Batches empty, so we've been cancelled
                var batches = []
            } else {
                var batches = mopidy.enqueue_uris_batches
                batches.shift()
            }
            return Object.assign({}, mopidy, {
                enqueue_uris_batches: batches  
            });


        /**
         * Library
         **/

        case 'MOPIDY_LIBRARY_PLAYLISTS_LOADED':
            if (mopidy.library_playlists){
                var uris = [...mopidy.library_playlists,...action.uris]
            } else {
                var uris = action.uris
            }
            return Object.assign({}, mopidy, { library_playlists: uris })

        case 'MOPIDY_LIBRARY_ARTISTS_LOADED':
            if (mopidy.library_artists){
                var uris = [...mopidy.library_artists,...action.uris]
            } else {
                var uris = action.uris
            }
            return Object.assign({}, mopidy, { library_artists: uris })

        case 'MOPIDY_LIBRARY_ALBUMS_LOADED':
            if (mopidy.library_albums){
                var uris = [...mopidy.library_albums,...action.uris]
            } else {
                var uris = action.uris
            }
            return Object.assign({}, mopidy, { library_albums: uris })


        case 'MOPIDY_LIBRARY_PLAYLISTS_CLEAR':
            return Object.assign({}, mopidy, { library_playlists: [] })

        case 'MOPIDY_LIBRARY_ARTISTS_CLEAR':
            return Object.assign({}, mopidy, { library_artists: [] })

        case 'MOPIDY_LIBRARY_ALBUMS_CLEAR':
            return Object.assign({}, mopidy, { library_albums: [] })


        case 'MOPIDY_LIBRARY_PLAYLISTS_PROCESSOR_CANCELLED':
            return Object.assign({}, mopidy, { library_playlists_status: 'cancelled' })

        case 'MOPIDY_LIBRARY_PLAYLISTS_PROCESSOR_FINISHED':
            return Object.assign({}, mopidy, { library_playlists_status: 'finished' })

        case 'MOPIDY_LIBRARY_ARTISTS_PROCESSOR_CANCELLED':
            return Object.assign({}, mopidy, { library_artists_status: 'cancelled' })

        case 'MOPIDY_LIBRARY_ARTISTS_PROCESSOR_FINISHED':
            return Object.assign({}, mopidy, { library_artists_status: 'finished' })

        case 'MOPIDY_LIBRARY_ALBUMS_PROCESSOR_CANCELLED':
            return Object.assign({}, mopidy, { library_albums_status: 'cancelled' })

        case 'MOPIDY_LIBRARY_ALBUMS_PROCESSOR_FINISHED':
            return Object.assign({}, mopidy, { library_albums_status: 'finished' })

        default:
            return mopidy
    }
}




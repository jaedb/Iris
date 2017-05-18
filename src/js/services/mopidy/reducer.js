
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

        case 'MOPIDY_ENQUEUE_URIS_CANCEL':
            return Object.assign({}, mopidy, {
                enqueue_uris_batches: []  
            });

        case 'MOPIDY_ENQUEUE_URIS_BATCH_DONE':
            if (!mopidy.enqueue_uris_batches || mopidy.enqueue_uris_batches.length <= 0){
                var batches = []
                console.error('Cannot remove batch when queue empty',action)
            } else {
                var batches = mopidy.enqueue_uris_batches
                batches.shift()
            }
            return Object.assign({}, mopidy, {
                enqueue_uris_batches: batches  
            });

        default:
            return mopidy
    }
}




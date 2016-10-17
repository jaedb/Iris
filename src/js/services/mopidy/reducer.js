
export default function reducer(mopidy = {}, action){
    switch (action.type) {

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

        /**
         * Websocket-initiated actions
         **/
        case 'MOPIDY_State':
            return Object.assign({}, mopidy, {
                state: action.data 
            });

        case 'MOPIDY_Consume':
            return Object.assign({}, mopidy, {
                consume: action.data 
            });

        case 'MOPIDY_Random':
            return Object.assign({}, mopidy, {
                random: action.data 
            });

        case 'MOPIDY_Repeat':
            return Object.assign({}, mopidy, {
                repeat: action.data 
            });

        case 'MOPIDY_TlTracks':
            return Object.assign({}, mopidy, {
                tracks: action.data 
            });

        case 'MOPIDY_CurrentTlTrack':
            return Object.assign({}, mopidy, {
                trackInFocus: action.data   
            });

        case 'MOPIDY_Volume':
            return Object.assign({}, mopidy, {
                volume: action.data   
            });

        default:
            return mopidy
    }
}




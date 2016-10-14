
export default function reducer(mopidy = {}, action){
    switch (action.type) {

        case 'CONNECTING':
            return Object.assign({}, mopidy, { connected: false, connecting: true });

        case 'CONNECTED':
            return Object.assign({}, mopidy, { connected: true, connecting: false });

        case 'DISCONNECTED':
            return Object.assign({}, mopidy, { connected: false, connecting: false });

        case 'State':
            return Object.assign({}, mopidy, {
                state: action.data 
            });

        case 'Consume':
            return Object.assign({}, mopidy, {
                consume: action.data 
            });

        case 'Random':
            return Object.assign({}, mopidy, {
                random: action.data 
            });

        case 'Repeat':
            return Object.assign({}, mopidy, {
                repeat: action.data 
            });

        case 'TlTracks':
            return Object.assign({}, mopidy, {
            	tracks: action.data	
            });

        case 'CurrentTlTrack':
            return Object.assign({}, mopidy, {
            	trackInFocus: action.data	
            });

        case 'Volume':
            return Object.assign({}, mopidy, {
                volume: action.volume   
            });

        case 'CHANGE_TRACK':
            return Object.assign({}, mopidy, {
            	tlid: action.tlid	
            });

        default:
            return mopidy
    }
}




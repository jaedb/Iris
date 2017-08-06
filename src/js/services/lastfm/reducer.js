
export default function reducer(lastfm = {}, action){
    switch (action.type) {

        case 'LASTFM_CONNECT':
        case 'LASTFM_CONNECTING':
            return Object.assign({}, lastfm, { connected: false, connecting: true });

        case 'LASTFM_CONNECTED':
            return Object.assign({}, lastfm, { connected: true, connecting: false });

        default:
            return lastfm
    }
}




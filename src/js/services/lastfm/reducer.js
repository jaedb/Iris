
export default function reducer(lastfm = {}, action){
    switch (action.type){

        case 'LASTFM_CONNECT':
        case 'LASTFM_CONNECTING':
            return Object.assign({}, lastfm, { connected: false, connecting: true });

        case 'LASTFM_CONNECTED':
            return Object.assign({}, lastfm, { connected: true, connecting: false });

        case 'LASTFM_SET':
            return Object.assign({}, lastfm, action.data)

        case 'LASTFM_AUTHORIZATION_GRANTED':
            return Object.assign({}, lastfm, {
                authorizing: false, 
                session: action.data.session
            })

        case 'LASTFM_AUTHORIZATION_REVOKED':
            return Object.assign({}, lastfm, { 
                authorizing: false,
                session: false,
                me: false
            });

        default:
            return lastfm
    }
}




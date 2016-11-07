
export default function reducer(lastfm = {}, action){
    switch (action.type) {

        case 'LASTFM_ARTIST_LOADED':
            return Object.assign({}, lastfm, { artist: action.data });

        case 'LASTFM_ALBUM_LOADED':
            return Object.assign({}, lastfm, { album: action.data });

        case 'LASTFM_TRACK_LOADED':
            return Object.assign({}, lastfm, { track: action.data });

        default:
            return lastfm
    }
}




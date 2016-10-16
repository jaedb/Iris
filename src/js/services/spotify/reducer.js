
export default function reducer(spotify = {}, action){
    switch (action.type) {

        case 'SPOTIFY_CONNECTING':
            return Object.assign({}, spotify, { connected: false, connecting: true });

        case 'SPOTIFY_START_AUTHORIZATION':
            return Object.assign({}, spotify, { authorizing: true });

        case 'SPOTIFY_COMPLETE_AUTHORIZATION':
            return Object.assign({}, spotify, { authorizing: false, authorization: action.data });

        case 'SPOTIFY_REMOVE_AUTHORIZATION':
            return Object.assign({}, spotify, { authorizing: false, authorization: false });

        case 'SPOTIFY_CONNECTED':
            return Object.assign({}, spotify, { connected: true, connecting: false });

        case 'SPOTIFY_DISCONNECTED':
            return Object.assign({}, spotify, { connected: false, connecting: false });

        case 'SPOTIFY_ALBUM_LOADED':
            return Object.assign({}, spotify, { album: action.data });

        case 'SPOTIFY_ARTIST_LOADED':
            return Object.assign({}, spotify, { artist: action.data });

        case 'SPOTIFY_LIBRARY_ARTISTS_LOADED':
            return Object.assign({}, spotify, { library_artists: action.data });

        default:
            return spotify
    }
}




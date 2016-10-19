
export default function reducer(spotify = {}, action){
    switch (action.type) {

        case 'SPOTIFY_CONNECTING':
            return Object.assign({}, spotify, { connected: false, connecting: true });

        case 'SPOTIFY_START_AUTHORIZATION':
            return Object.assign({}, spotify, { authorizing: true });

        case 'SPOTIFY_AUTHORIZATION_COMPLETE':
            return Object.assign({}, spotify, { 
                authorizing: false, 
                authorized: true,
                authorization: action.data,
                me: action.data.me,
                token: action.data.access_token
            });

        case 'SPOTIFY_REMOVE_AUTHORIZATION':
            return Object.assign({}, spotify, { 
                authorizing: false, 
                authorization: false,
                authorized: false,
                token: false,
                me: false
            });

        case 'SPOTIFY_CONNECTED':
            return Object.assign({}, spotify, { connected: true, connecting: false });

        case 'SPOTIFY_DISCONNECTED':
            return Object.assign({}, spotify, { connected: false, connecting: false });

        case 'SPOTIFY_ALBUM_LOADED':
            return Object.assign({}, spotify, { album: action.data });

        case 'SPOTIFY_ARTIST_LOADED':
            return Object.assign({}, spotify, { artist: action.data });

        case 'SPOTIFY_ARTIST_ALBUMS_LOADED':            
            return Object.assign({}, spotify, { artist_albums: action.data });

        case 'SPOTIFY_ARTIST_TRACKS_LOADED':
            return Object.assign({}, spotify, { artist_top_tracks: action.data });

        case 'SPOTIFY_LIBRARY_ARTISTS_LOADED':
            return Object.assign({}, spotify, { libraryArtists: action.data });

        default:
            return spotify
    }
}





export default function reducer(spotify = {}, action){
    switch (action.type) {

        case 'SPOTIFY_CONNECTING':
            return Object.assign({}, spotify, { connected: false, connecting: true });

        case 'SPOTIFY_CONNECTED':
            return Object.assign({}, spotify, { connected: true, connecting: false });

        case 'SPOTIFY_AUTHORIZATION_GRANTED':
            return Object.assign({}, spotify, { 
                authorizing: false, 
                authorized: true,
                authorization: action.data,
                access_token: action.data.access_token,
                token_expiry: action.data.token_expiry
            });

        case 'SPOTIFY_REMOVE_AUTHORIZATION':
            return Object.assign({}, spotify, { 
                authorizing: false, 
                authorization: false,
                authorized: false,
                access_token: false,
                me: false
            });

        case 'SPOTIFY_TOKEN_REFRESHING':
            return Object.assign({}, spotify, { refreshing_token: true });

        case 'SPOTIFY_TOKEN_REFRESHED':
            return Object.assign({}, spotify, {
                refreshing_token: false,
                authorization: action.data,
                access_token: action.data.access_token,
                token_expiry: action.data.token_expiry
            });

        case 'SPOTIFY_DISCONNECTED':
            return Object.assign({}, spotify, { connected: false, connecting: false });

        case 'SPOTIFY_ME_LOADED':
            return Object.assign({}, spotify, { me: action.data });

        case 'SPOTIFY_PLAYLIST_LOADED':
            return Object.assign({}, spotify, { playlist: action.data });

        case 'SPOTIFY_ALBUM_LOADED':
            return Object.assign({}, spotify, { album: action.data });

        case 'SPOTIFY_ARTIST_LOADED':
            return Object.assign({}, spotify, { artist: action.data });

        case 'SPOTIFY_ARTIST_ALBUMS_LOADED':            
            return Object.assign({}, spotify, { artist_albums: action.data });

        case 'SPOTIFY_LIBRARY_PLAYLISTS_LOADED':
            return Object.assign({}, spotify, { libraryPlaylists: action.data });

        case 'SPOTIFY_LIBRARY_ARTISTS_LOADED':
            return Object.assign({}, spotify, { libraryArtists: action.data });

        case 'SPOTIFY_LIBRARY_ALBUMS_LOADED':
            return Object.assign({}, spotify, { libraryAlbums: action.data });

        case 'SPOTIFY_LIBRARY_TRACKS_LOADED':
            return Object.assign({}, spotify, { libraryTracks: action.data });

        default:
            return spotify
    }
}




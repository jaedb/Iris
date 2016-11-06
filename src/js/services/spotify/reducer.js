
export default function reducer(spotify = {}, action){
    switch (action.type) {

        case 'SPOTIFY_CONNECT':
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
                refresh_token: action.data.refresh_token,
                token_expiry: action.data.token_expiry
            });

        case 'PUSHER_SPOTIFY_TOKEN':
            if( spotify.authorized ) return spotify;
            return Object.assign({}, spotify, { 
                authorizing: false, 
                authorized: false,
                authorization: false,
                access_token: action.data.access_token,
                token_expiry: action.data.token_expiry
            });

        case 'SPOTIFY_AUTHORIZATION_REVOKED':
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

        case 'SPOTIFY_TRACK_LOADED':
            return Object.assign({}, spotify, { track: action.data });

        case 'SPOTIFY_PLAYLIST_LOADED':
            return Object.assign({}, spotify, { playlist: action.data });

        case 'SPOTIFY_ALBUM_LOADED':
            return Object.assign({}, spotify, { album: action.data });

        case 'SPOTIFY_ARTIST_LOADED':
            return Object.assign({}, spotify, { artist: action.data });

        case 'SPOTIFY_ARTISTS_LOADED':
            return Object.assign({}, spotify, { artists: action.data });

        case 'SPOTIFY_ARTIST_ALBUMS_LOADED':            
            return Object.assign({}, spotify, { artist_albums: action.data });

        case 'SPOTIFY_ARTIST_ALBUMS_LOADED_MORE':
            return Object.assign({}, spotify, { artist_albums: {
                href: action.data.href,
                next: action.data.next,
                previous: action.data.previous,
                items: [ ...spotify.artist_albums.items, ...action.data.items ]
            }});

        case 'SPOTIFY_LIBRARY_PLAYLISTS_LOADED':
            return Object.assign({}, spotify, { library_playlists: action.data });

        case 'SPOTIFY_LIBRARY_ARTISTS_LOADED':
            return Object.assign({}, spotify, { library_artists: action.data });

        case 'SPOTIFY_LIBRARY_ARTISTS_LOADED_MORE':
            return Object.assign({}, spotify, { library_artists: {
                href: action.data.artists.href,
                next: action.data.artists.next,
                previous: action.data.artists.previous,
                items: [ ...spotify.library_artists.items, ...action.data.artists.items ]
            }});

        case 'SPOTIFY_LIBRARY_ALBUMS_LOADED':
            return Object.assign({}, spotify, { library_albums: action.data });

        case 'SPOTIFY_LIBRARY_ALBUMS_LOADED_MORE':
            return Object.assign({}, spotify, { library_albums: {
                href: action.data.href,
                next: action.data.next,
                previous: action.data.previous,
                items: [ ...spotify.library_albums.items, ...action.data.items ]
            }});

        case 'SPOTIFY_LIBRARY_TRACKS_LOADED':
            return Object.assign({}, spotify, { library_tracks: action.data });

        case 'SPOTIFY_FEATURED_PLAYLISTS_LOADED':
            return Object.assign({}, spotify, { featured_playlists: action.data });

        case 'SPOTIFY_CATEGORIES_LOADED':
            return Object.assign({}, spotify, { categories: action.data });

        case 'SPOTIFY_CATEGORY_LOADED':
            return Object.assign({}, spotify, { category: action.data });

        case 'SPOTIFY_CATEGORY_PLAYLISTS_LOADED':
            return Object.assign({}, spotify, { category_playlists: action.data });

        case 'SPOTIFY_CATEGORY_PLAYLISTS_LOADED_MORE':
            return Object.assign({}, spotify, { category_playlists: {
                href: action.data.href,
                next: action.data.next,
                previous: action.data.previous,
                items: [ ...spotify.category_playlists.items, ...action.data.items ]
            }});

        case 'SPOTIFY_NEW_RELEASES_LOADED':
            return Object.assign({}, spotify, { new_releases: action.data });

        case 'SPOTIFY_NEW_RELEASES_LOADED_MORE':
            return Object.assign({}, spotify, { new_releases: {
                href: action.data.albums.href,
                next: action.data.albums.next,
                previous: action.data.albums.previous,
                items: [ ...spotify.new_releases.items, ...action.data.albums.items ]
            }});

        case 'SPOTIFY_SEARCH_RESULTS_LOADED':
            return Object.assign({}, spotify, { 
                search_results_artists: action.data.artists,
                search_results_albums: action.data.albums,
                search_results_playlists: action.data.playlists,
                search_results_tracks: action.data.tracks
            });

        case 'SPOTIFY_SEARCH_RESULTS_LOADED_MORE_ARTISTS':
            return Object.assign({}, spotify, { search_results_artists: {
                href: action.data.artists.href,
                next: action.data.artists.next,
                previous: action.data.artists.previous,
                items: [ ...spotify.search_results_artists.items, ...action.data.artists.items ]
            }});

        case 'SPOTIFY_SEARCH_RESULTS_LOADED_MORE_ALBUMS':
            return Object.assign({}, spotify, { search_results_albums: {
                href: action.data.albums.href,
                next: action.data.albums.next,
                previous: action.data.albums.previous,
                items: [ ...spotify.search_results_albums.items, ...action.data.albums.items ]
            }});

        case 'SPOTIFY_SEARCH_RESULTS_LOADED_MORE_PLAYLISTS':
            return Object.assign({}, spotify, { search_results_playlists: {
                href: action.data.playlists.href,
                next: action.data.playlists.next,
                previous: action.data.playlists.previous,
                items: [ ...spotify.search_results_playlists.items, ...action.data.playlists.items ]
            }});

        case 'SPOTIFY_SEARCH_RESULTS_LOADED_MORE_TRACKS':
            return Object.assign({}, spotify, { search_results_tracks: {
                href: action.data.tracks.href,
                next: action.data.tracks.next,
                previous: action.data.tracks.previous,
                items: [ ...spotify.search_results_tracks.items, ...action.data.tracks.items ]
            }});

        default:
            return spotify
    }
}




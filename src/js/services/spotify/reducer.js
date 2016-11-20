
export default function reducer(spotify = {}, action){
    switch (action.type) {

        case 'SPOTIFY_CONNECT':
        case 'SPOTIFY_CONNECTING':
            return Object.assign({}, spotify, { connected: false, connecting: true });

        case 'SPOTIFY_CONNECTED':
            return Object.assign({}, spotify, { connected: true, connecting: false });

        case 'PUSHER_SPOTIFY_TOKEN':
            if( spotify.authorized ) return spotify;
            return Object.assign({}, spotify, { 
                authorizing: false, 
                authorized: false,
                authorization: false,
                access_token: action.data.access_token,
                token_expiry: action.data.token_expiry
            });

        case 'SPOTIFY_AUTHORIZATION_GRANTED':
            return Object.assign({}, spotify, { 
                authorizing: false, 
                authorized: true,
                authorization: action.data,
                access_token: action.data.access_token,
                refresh_token: action.data.refresh_token,
                token_expiry: action.data.token_expiry
            });

        case 'SPOTIFY_AUTHORIZATION_REVOKED':
            return Object.assign({}, spotify, { 
                authorizing: false, 
                authorization: false,
                authorized: false,
                access_token: false,
                refresh_token: false,
                token_expiry: 0,
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

        case 'SPOTIFY_ARTISTS_LOADED':
            if( !action.data ) return Object.assign({}, spotify)
            return Object.assign({}, spotify, {
                artists: action.data.artists.items,
                artists_more: action.data.artists.next
            });

        case 'SPOTIFY_LIBRARY_ARTISTS_LOADED':
            if( !action.data ) return Object.assign({}, spotify)
            return Object.assign({}, spotify, {
                library_artists: action.data.artists.items,
                library_artists_more: action.data.artists.next
            });

        case 'SPOTIFY_LIBRARY_ARTISTS_LOADED_MORE':
            return Object.assign({}, spotify, { 
                library_artists: [ ...spotify.library_artists, ...action.data.artists.items ],
                library_artists_more: action.data.artists.next
            });

        case 'SPOTIFY_LIBRARY_ALBUMS_LOADED':
            if( !action.data ) return Object.assign({}, spotify)
            return Object.assign({}, spotify, { 
                library_albums: action.data.items, 
                library_albums_more: action.data.next 
            });

        case 'SPOTIFY_LIBRARY_ALBUMS_LOADED_MORE':
            return Object.assign({}, spotify, {
                library_albums: [...spotify.library_albums, ...action.data.items ],
                library_albums_more: action.data.next
            });

        case 'SPOTIFY_LIBRARY_TRACKS_LOADED':
            if( !action.data ) return Object.assign({}, spotify)
            var tracks = Object.assign([], action.data.items)
            for( var i = 0; i < tracks.length; i++ ){
                tracks[i] = Object.assign(
                    {},
                    tracks[i].track,
                    {
                        added_at: tracks[i].added_at
                    }
                )
            }
            return Object.assign({}, spotify, { 
                library_tracks: tracks, 
                library_tracks_more: action.data.next 
            });

        case 'SPOTIFY_LIBRARY_TRACKS_LOADED_MORE':
            var tracks = Object.assign([], action.data.items)
            for( var i = 0; i < tracks.length; i++ ){
                tracks[i] = Object.assign(
                    {},
                    tracks[i].track,
                    {
                        added_at: tracks[i].added_at
                    }
                )
            }
            return Object.assign({}, spotify, { 
                library_tracks: [...spotify.library_tracks, ...tracks], 
                library_tracks_more: action.data.next 
            });

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

        default:
            return spotify
    }
}




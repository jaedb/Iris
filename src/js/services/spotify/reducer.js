
import * as helpers from '../../helpers'

export default function reducer(spotify = {}, action){
    switch (action.type) {

        case 'SPOTIFY_CONNECT':
        case 'SPOTIFY_CONNECTING':
            return Object.assign({}, spotify, { connected: false, connecting: true })

        case 'SPOTIFY_CONNECTED':
            return Object.assign({}, spotify, { connected: true, connecting: false })

        case 'SPOTIFY_DISCONNECTED':
            return Object.assign({}, spotify, { connected: false, connecting: false })

        case 'SPOTIFY_SET_CONFIG':
            return Object.assign({}, spotify, action.config)

        case 'PUSHER_SPOTIFY_TOKEN':
            if( spotify.authorized ) return spotify;
            return Object.assign({}, spotify, { 
                authorizing: false, 
                authorized: false,
                authorization: false,
                access_token: action.data.access_token,
                token_expiry: action.data.token_expiry
            })

        case 'SPOTIFY_AUTHORIZATION_GRANTED':
            return Object.assign({}, spotify, { 
                authorizing: false, 
                authorized: true,
                authorization: action.data,
                access_token: action.data.access_token,
                refresh_token: action.data.refresh_token,
                token_expiry: action.data.token_expiry
            })

        case 'SPOTIFY_AUTHORIZATION_REVOKED':
            return Object.assign({}, spotify, { 
                authorizing: false, 
                authorization: false,
                authorized: false,
                access_token: false,
                refresh_token: false,
                token_expiry: 0,
                me: false
            })

        case 'SPOTIFY_IMPORT_AUTHORIZATION':
            return Object.assign({}, spotify, { 
                authorizing: false, 
                authorized: true,
                authorization: action.authorization,
                access_token: action.authorization.access_token,
                refresh_token: action.authorization.refresh_token,
                token_expiry: action.authorization.token_expiry,
                me: action.user
            })

        case 'SPOTIFY_TOKEN_REFRESHING':
            return Object.assign({}, spotify, { refreshing_token: true })

        case 'SPOTIFY_TOKEN_REFRESHED':
            return Object.assign({}, spotify, {
                connected: true,
                refreshing_token: false,
                access_token: action.data.access_token,
                token_expiry: action.data.token_expiry,
                provider: action.provider
            })

        case 'SPOTIFY_DISCONNECTED':
            return Object.assign({}, spotify, { connected: false, connecting: false })

        case 'SPOTIFY_ME_LOADED':
            return Object.assign({}, spotify, { me: action.data })
/*
        case 'SPOTIFY_ARTISTS_LOADED':
            if( !action.data ) return Object.assign({}, spotify)
            return Object.assign({}, spotify, {
                artists: action.data.artists.items,
                artists_more: action.data.artists.next
            })

        case 'SPOTIFY_LIBRARY_ARTISTS_LOADED':
            if( !action.data ) return Object.assign({}, spotify)
            return Object.assign({}, spotify, {
                library_artists: action.data.artists.items,
                library_artists_more: action.data.artists.next
            })

        case 'SPOTIFY_LIBRARY_ARTISTS_LOADED_MORE':
            return Object.assign({}, spotify, { 
                library_artists: [ ...spotify.library_artists, ...action.data.artists.items ],
                library_artists_more: action.data.artists.next
            })*/

        case 'SPOTIFY_LIBRARY_ALBUMS_LOADED':
            if( !action.data ) return Object.assign({}, spotify)
            var albums = []
            for( var i = 0; i < action.data.items.length; i++ ){
                albums.push( Object.assign(
                    {},
                    action.data.items[i].album,
                    {
                        added_at: action.data.items[i].added_at
                    }
                ))
            }
            return Object.assign({}, spotify, { 
                library_albums: albums, 
                library_albums_more: action.data.next 
            })

        case 'SPOTIFY_LIBRARY_ALBUMS_LOADED_MORE':
            var albums = []
            for( var i = 0; i < action.data.items.length; i++ ){
                albums.push( Object.assign(
                    {},
                    action.data.items[i].album,
                    {
                        added_at: action.data.items[i].added_at
                    }
                ))
            }
            return Object.assign({}, spotify, {
                library_albums: [...spotify.library_albums, ...albums ],
                library_albums_more: action.data.next
            })

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
            })

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
            })

        case 'SPOTIFY_FEATURED_PLAYLISTS_LOADED':
            return Object.assign({}, spotify, { featured_playlists: action.data })

        case 'SPOTIFY_NEW_RELEASES_LOADED':
            return Object.assign({}, spotify, { new_releases: action.data });

        case 'SPOTIFY_NEW_RELEASES_LOADED_MORE':
            return Object.assign({}, spotify, { new_releases: {
                href: action.data.albums.href,
                next: action.data.albums.next,
                previous: action.data.albums.previous,
                items: [ ...spotify.new_releases.items, ...action.data.albums.items ]
            }})

        case 'SPOTIFY_DISCOVER_LOADED':
            if( !action.data ) return Object.assign({}, spotify, { discover: [] })
            return Object.assign({}, spotify, { discover: [...spotify.discover, ...[action.data]] })

        case 'SPOTIFY_RECOMMENDATIONS_LOADED':
            return Object.assign(
                {}, 
                spotify, 
                {
                    recommendations: {
                        artists_uris: action.artists_uris,
                        albums_uris: action.albums_uris,
                        tracks: action.tracks
                    }
                })

        case 'SPOTIFY_FAVORITES_LOADED':
            return Object.assign(
                {}, 
                spotify, 
                {
                    favorite_artists: action.artists_uris,
                    favorite_tracks: action.tracks_uris
                })

        case 'SPOTIFY_AUTOCOMPLETE_LOADING':
            var autocomplete_results = spotify.autocomplete_results
            autocomplete_results[action.field_id] = {loading: true}
            return Object.assign(
                {}, 
                spotify, 
                {
                    autocomplete_results: autocomplete_results
                })

        case 'SPOTIFY_AUTOCOMPLETE_LOADED':
            var autocomplete_results = spotify.autocomplete_results
            autocomplete_results[action.field_id] = action.results
            autocomplete_results[action.field_id].loading = false
            return Object.assign(
                {}, 
                spotify, 
                {
                    autocomplete_results: autocomplete_results
                })

        case 'SPOTIFY_AUTOCOMPLETE_CLEAR':
            var autocomplete_results = spotify.autocomplete_results
            if (typeof(autocomplete_results[action.field_id]) !== 'undefined'){
                delete autocomplete_results[action.field_id]
            }
            return Object.assign(
                {}, 
                spotify, 
                {
                    autocomplete_results: autocomplete_results
                })

        default:
            return spotify
    }
}




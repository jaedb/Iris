
import * as helpers from '../../helpers'

export default function reducer(spotify = {}, action){
    switch (action.type){

        case 'SPOTIFY_SET':
            return Object.assign({},spotify,action.data)

        case 'PUSHER_SPOTIFY_TOKEN':
            if (spotify.authorization) return spotify;
            return Object.assign({}, spotify, { 
                authorizing: false, 
                authorization: false,
                access_token: action.data.access_token,
                token_expiry: action.data.token_expiry
            })

        case 'SPOTIFY_AUTHORIZATION_GRANTED':
            return Object.assign({}, spotify, { 
                enabled: true, 
                authorizing: false, 
                authorization: action.data,
                access_token: action.data.access_token,
                refresh_token: action.data.refresh_token,
                token_expiry: action.data.token_expiry
            })

        case 'SPOTIFY_AUTHORIZATION_REVOKED':
            return Object.assign({}, spotify, { 
                authorizing: false, 
                authorization: false,
                access_token: false,
                refresh_token: false,
                token_expiry: 0,
                me: false
            })

        case 'SPOTIFY_IMPORT_AUTHORIZATION':
            return Object.assign({}, spotify, {
                authorizing: false, 
                authorization: action.authorization,
                access_token: action.authorization.access_token,
                refresh_token: action.authorization.refresh_token,
                token_expiry: action.authorization.token_expiry,
                me: action.me
            });

        case 'SPOTIFY_TOKEN_REFRESHING':
            return Object.assign({}, spotify, { refreshing_token: true })

        case 'SPOTIFY_TOKEN_REFRESHED':
            return Object.assign({}, spotify, {
                refreshing_token: false,
                access_token: action.data.access_token,
                token_expiry: action.data.token_expiry
            });

        case 'SPOTIFY_TOKEN_CHANGED':
            return Object.assign({}, spotify, {
                access_token: action.spotify_token.access_token,
                token_expiry: action.spotify_token.token_expiry
            });

        case 'SPOTIFY_ME_LOADED':
            return Object.assign({}, spotify, { me: action.me })

        case 'SPOTIFY_FEATURED_PLAYLISTS_LOADED':
            return Object.assign({}, spotify, { featured_playlists: action.data })


        case 'SPOTIFY_NEW_RELEASES_LOADED':
            var new_releases = [];
            if (spotify.new_releases){
            	new_releases = Object.assign([], spotify.new_releases);
            }
            return Object.assign({}, spotify, { 
                new_releases: helpers.removeDuplicates([...new_releases, ...action.uris]),
                new_releases_more: action.more,
                new_releases_total: action.total
            });

        case 'SPOTIFY_DISCOVER_LOADED':
            if (!action.data ){
                return Object.assign(
                    {}, 
                    spotify, 
                    { 
                        discover: []
                    }
                );
            }
            return Object.assign(
                {}, 
                spotify, 
                { 
                    discover: [...spotify.discover, ...[action.data]]
                }
            );

        case 'CLEAR_SPOTIFY_RECOMMENDATIONS':
            return Object.assign({}, spotify, {recommendations: {artists_uris: [], albums_uris: [], tracks_uris: []}});

        case 'SPOTIFY_RECOMMENDATIONS_LOADED':
            return Object.assign(
                {}, 
                spotify, 
                {
                    recommendations: {
                        artists_uris: action.artists_uris,
                        albums_uris: action.albums_uris,
                        tracks_uris: action.tracks_uris
                    }
                });

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

        case 'SPOTIFY_GENRES_LOADED':
            return Object.assign(
                {}, 
                spotify, 
                {
                    genres: action.genres
                })


        /**
         * Categories
         **/

        case 'SPOTIFY_CATEGORIES_LOADED':
            var categories = Object.assign({}, spotify.categories);
            for (var category of action.categories){
                categories[category.uri] = category
            }
            return Object.assign({}, spotify, { categories: categories });

        case 'SPOTIFY_CATEGORY_PLAYLISTS_LOADED':
            var categories = Object.assign({}, spotify.categories)
            var playlists_uris = [];

            if (categories[action.uri].playlists_uris){
                playlists_uris = categories[action.uri].playlists_uris;
            }

            var category = Object.assign(
                {}, 
                categories[action.uri],
                {
                    playlists_uris: [...playlists_uris, ...action.uris],
                    playlists_more: action.more,
                    playlists_total: action.total
                }
            )
            categories[action.uri] = category
            return Object.assign({}, spotify, { categories: categories });



        /**
         * Library
         **/

        case 'SPOTIFY_FLUSH_LIBRARY':
            return Object.assign(
                {}, 
                spotify, 
                { 
                    library_playlists: null,
                    library_playlists_loaded_all: null,
                    library_playlists_status: null,
                    library_albums: null,
                    library_albums_status: null,
                    library_albums_loaded_all: null,
                    library_artists: null,
                    library_artists_status: null,
                    library_artists_loaded_all: null,
                    library_tracks: null,
                    library_tracks_status: null,
                    library_tracks_loaded_all: null
                }
            )

        case 'SPOTIFY_LIBRARY_PLAYLISTS_LOADED':
            if (spotify.library_playlists){
                var uris = [...spotify.library_playlists,...action.uris]
            } else {
                var uris = action.uris
            }
            return Object.assign({}, spotify, { library_playlists: helpers.removeDuplicates(uris) })

        case 'SPOTIFY_LIBRARY_ARTISTS_LOADED':
            if (spotify.library_artists){
                var uris = [...spotify.library_artists,...action.uris]
            } else {
                var uris = action.uris
            }
            return Object.assign({}, spotify, { library_artists: helpers.removeDuplicates(uris) })

        case 'SPOTIFY_LIBRARY_ALBUMS_LOADED':
            if (spotify.library_albums){
                var uris = [...spotify.library_albums,...action.uris]
            } else {
                var uris = action.uris
            }
            return Object.assign({}, spotify, { library_albums: helpers.removeDuplicates(uris) })

        case 'SPOTIFY_LIBRARY_TRACKS_LOADED':
        case 'SPOTIFY_LIBRARY_TRACKS_LOADED_MORE':
            var tracks = action.data.items;
            var uris = [];

            if (tracks){
                tracks = helpers.formatTracks(tracks);
                uris = helpers.arrayOf('uri', tracks);
                if (spotify.library_tracks){
                    uris = [...spotify.library_tracks, ...uris]
                }
            }

            return Object.assign({}, spotify, { 
                library_tracks: helpers.removeDuplicates(uris), 
                library_tracks_more: action.data.next
            })


        case 'SPOTIFY_LIBRARY_PLAYLISTS_LOADED_ALL':
            return Object.assign({}, spotify, { library_playlists_loaded_all: true })

        case 'SPOTIFY_LIBRARY_PLAYLISTS_CLEAR':
            return Object.assign({}, spotify, { library_playlists: [] })

        case 'SPOTIFY_LIBRARY_ARTISTS_CLEAR':
            return Object.assign({}, spotify, { library_artists: [] })

        case 'SPOTIFY_LIBRARY_ALBUMS_CLEAR':
            return Object.assign({}, spotify, { library_albums: [] })


        case 'SPOTIFY_LIBRARY_ALBUM_CHECK':
            var items = Object.assign([], spotify.library_albums)
            var index = items.indexOf(action.key)
            if (index > -1 && !action.in_library){
                items.splice(index, 1)
            } else if (index < 0 && action.in_library){
                items.push(action.key)
            }
            return Object.assign({}, spotify, { library_albums: items });

        case 'SPOTIFY_LIBRARY_ARTIST_CHECK':
            var items = Object.assign([], spotify.library_artists)
            var index = items.indexOf(action.key)
            if (index > -1 && !action.in_library){
                items.splice(index, 1)
            } else if (index < 0 && action.in_library){
                items.push(action.key)
            }
            return Object.assign({}, spotify, { library_artists: items });

        case 'SPOTIFY_LIBRARY_PLAYLIST_CHECK':
            var items = Object.assign([], spotify.library_playlists)
            var index = items.indexOf(action.key)
            if (index > -1 && !action.in_library){
                items.splice(index, 1)
            } else if (index < 0 && action.in_library){
                items.push(action.key)
            }
            return Object.assign({}, spotify, { library_playlists: items });

        case 'SPOTIFY_LIBRARY_TRACK_CHECK':
            var items = Object.assign([], spotify.library_tracks)
            var index = items.indexOf(action.key)
            if (index > -1 && !action.in_library) {
                items.splice(index, 1)
            } else if (index < 0 && action.in_library){
                items.push(action.key)
            }
            return Object.assign({}, spotify, { library_tracks: items });


        /**
         * Searching
         **/

        case 'SPOTIFY_CLEAR_SEARCH_RESULTS':
            return Object.assign({}, spotify, { search_results: {} });

        case 'SPOTIFY_SEARCH_RESULTS_LOADED':

            // Fetch or create our container
            if (spotify.search_results){
                var search_results = Object.assign({}, spotify.search_results)
            } else {
                var search_results = {}
            }

            search_results.query = action.query;

            if (search_results.results){
                search_results[action.context] = [...search_results[action.context], ...action.results]
            } else {
                search_results[action.context] = action.results
            }

            if (action.more){
                search_results[action.context+'_more'] = action.more
            } else {
                search_results[action.context+'_more'] = null
            }
            return Object.assign({}, spotify, { search_results: search_results });

        default:
            return spotify
    }
}




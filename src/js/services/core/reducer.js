
import * as helpers from '../../helpers'

export default function reducer(core = {}, action){
    switch (action.type){

        case 'CORE_SET':
            return Object.assign({}, core, action.data)

        case 'CACHEBUST_HTTP_STREAM':
            return Object.assign({}, core, {http_streaming_cachebuster: new Date().getTime()})

        /**
         * Current track and tracklist
         **/

        case 'CURRENT_TRACK_LOADED':
            return Object.assign({}, core, {
                current_track: action.track,
                current_track_uri: action.uri
            });

        case 'CLEAR_CURRENT_TRACK':
            return Object.assign({}, core, {
                current_track: null,
                current_track_uri: null
            });

        case 'NEXT_TRACK_LOADED':
            return Object.assign({}, core, {
                next_track_uri: action.uri
            });

        case 'QUEUE_LOADED':
            return Object.assign({}, core, {
                queue: action.tracks
            });

        case 'PUSHER_QUEUE_METADATA':
        case 'PUSHER_QUEUE_METADATA_CHANGED':
            var tracklist = Object.assign([], core.current_tracklist)
            for(var i = 0; i < tracklist.length; i++){

                // load our metadata (if we have any for that tlid)
                if (action.queue_metadata['tlid_'+tracklist[i].tlid] !== undefined){
                    tracklist[i] = Object.assign(
                        {},
                        tracklist[i],
                        action.queue_metadata['tlid_'+tracklist[i].tlid],
                    )
                }
            }
            return Object.assign({}, core, { current_tracklist: tracklist, queue_metadata: action.queue_metadata });

        case 'PUSHER_RADIO_LOADED':
        case 'PUSHER_RADIO_STARTED':
        case 'PUSHER_RADIO_CHANGED':
        case 'PUSHER_RADIO_STOPPED':
            return Object.assign({}, core, { seeds_resolved: false }, { radio: action.radio })

        case 'RADIO_SEEDS_RESOLVED':
            var radio = Object.assign({}, core.radio, { resolved_seeds: action.resolved_seeds })
            return Object.assign({}, core, { radio: radio })






        /**
         * Index updates
         * These actions are only ever called by middleware after we've digested one more many assets
         * and appended to their relevant index.
         **/

        case 'TRACKS_LOADED':
            var tracks = Object.assign({}, core.tracks);
            for (var track of action.tracks){
                tracks[track.uri] = track;
            }
            return Object.assign({}, core, { tracks: tracks });

        case 'ALBUMS_LOADED':
            var albums = Object.assign({}, core.albums);
            for (var album of action.albums){
                albums[album.uri] = album;
            }
            return Object.assign({}, core, { albums: albums });

        case 'ARTISTS_LOADED':
            var artists = Object.assign({}, core.artists);
            for (var artist of action.artists){
                artists[artist.uri] = artist;
            }
            return Object.assign({}, core, { artists: artists });

        case 'PLAYLISTS_LOADED':
            var playlists = Object.assign({}, core.playlists);
            for (var playlist of action.playlists){
                playlists[playlist.uri] = playlist;
            }
            return Object.assign({}, core, { playlists: playlists });

        case 'USERS_LOADED':
            var users = Object.assign({}, core.users);
            for (var user of action.users){
                users[user.uri] = user;
            }
            return Object.assign({}, core, { users: users });


        case 'ARTIST_ALBUMS_LOADED':
            var artists = Object.assign({}, core.artists)
            var albums_uris = [];
            if (artists[action.artist_uri].albums_uris){
                albums_uris = artists[action.artist_uri].albums_uris;
            }

            var artist = Object.assign(
                {}, 
                artists[action.artist_uri],
                {
                    albums_uris: [...albums_uris, ...action.albums_uris],
                    albums_more: action.more,
                    albums_total: action.total
                }
            )
            artists[action.artist_uri] = artist
            return Object.assign({}, core, { artists: artists });


        case 'USER_PLAYLISTS_LOADED':
            var users = Object.assign({}, core.users)
            var existing_playlists_uris = [];
            if (users[action.uri] && users[action.uri].playlists_uris){
            	existing_playlists_uris = users[action.uri].playlists_uris;
            }

            var playlists_uris = [...existing_playlists_uris, ...helpers.arrayOf('uri',action.playlists)]

            var user = Object.assign(
                {}, 
                users[action.uri],
                {
                    playlists_uris: playlists_uris,
                    playlists_more: action.more,
                    playlists_total: action.total
                }
            )
            users[action.uri] = user;
            return Object.assign({}, core, { users: users });




        /**
         * Playlists
         **/

        case 'PLAYLIST_TRACKS':
            console.log(action)
            var playlists = Object.assign({}, core.playlists)
            var playlist = Object.assign({}, playlists[action.key], { tracks_uris: action.tracks_uris })

            playlists[action.key] = playlist
            return Object.assign({}, core, { playlists: playlists });

        case 'LIBRARY_PLAYLISTS_LOADED':
            if (core.library_playlists){
                var library_playlists = [...core.library_playlists, ...action.uris]
            } else {
                var library_playlists = action.uris
            }

            library_playlists = helpers.removeDuplicates(library_playlists)

            return Object.assign({}, core, { 
                library_playlists: library_playlists,
                library_playlists_started: true
            });


        /**
         * Genres
         **/

        case 'SPOTIFY_GENRES_LOADED':
            return Object.assign({}, core, {genres: action.genres})


        /**
         * Search results
         **/

        case 'SEARCH_STARTED':
            return Object.assign({}, core, {
                search_results: {
                    artists_uris: [],
                    albums_uris: [],
                    playlists_uris: [],
                    tracks: [],
                }
            });

        case 'SEARCH_RESULTS_LOADED':

            // artists
            if (core.search_results && core.search_results.artists_uris){
                var artists_uris = core.search_results.artists_uris
            } else {
                var artists_uris = []
            }
            if (action.artists_uris) artists_uris = [...artists_uris, ...action.artists_uris]

            // more tracks
            if (typeof(action.artists_more) !== 'undefined') var artists_more = action.artists_more
            else if (core.search_results && core.search_results.artists_more) var artists_more = core.search_results.artists_more
            else var artists_more = null


            // albums
            if (core.search_results && core.search_results.albums_uris){
                var albums_uris = core.search_results.albums_uris
            } else {
                var albums_uris = []
            }
            if (action.albums_uris) albums_uris = [...albums_uris, ...action.albums_uris]

            // more tracks
            if (typeof(action.albums_more) !== 'undefined') var albums_more = action.albums_more
            else if (core.search_results && core.search_results.albums_more) var albums_more = core.search_results.albums_more
            else var albums_more = null


            // playlists
            if (core.search_results && core.search_results.playlists_uris){
                var playlists_uris = core.search_results.playlists_uris
            } else {
                var playlists_uris = []
            }
            if (action.playlists_uris) playlists_uris = [...playlists_uris, ...action.playlists_uris]

            // more tracks
            if (typeof(action.playlists_more) !== 'undefined') var playlists_more = action.playlists_more
            else if (core.search_results && core.search_results.playlists_more) var playlists_more = core.search_results.playlists_more
            else var playlists_more = null


            // tracks
            if (core.search_results && core.search_results.tracks){
                var tracks = core.search_results.tracks
            } else {
                var tracks = []
            }
            if (action.tracks) tracks = [...tracks, ...helpers.formatTracks(action.tracks)]

            // more tracks
            if (typeof(action.tracks_more) !== 'undefined') var tracks_more = action.tracks_more
            else if (core.search_results && core.search_results.tracks_more) var tracks_more = core.search_results.tracks_more
            else var tracks_more = null

            return Object.assign({}, core, {
                search_results: {
                    artists_more: artists_more,
                    artists_uris: helpers.removeDuplicates(artists_uris),
                    albums_more: albums_more,
                    albums_uris: helpers.removeDuplicates(albums_uris),
                    playlists_more: playlists_more,
                    playlists_uris: helpers.removeDuplicates(playlists_uris),
                    tracks: tracks,
                    tracks_more: tracks_more
                }
            });


        default:
            return core
    }
}




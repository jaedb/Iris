
import * as helpers from '../../helpers'

export default function reducer(core = {}, action){
    switch (action.type){

        case 'CORE_SET':
            return Object.assign({}, core, action.data)

        /**
         * Current track and tracklist
         **/

        case 'MOPIDY_TLTRACKS':
            if (!action.data ) return core

            var tracklist = []
            for (var i = 0; i < action.data.length; i++){

                var tltrack = helpers.formatTracks(action.data[i]);

                // load our metadata (if we have any for that tlid)
                if (core.queue_metadata !== undefined && core.queue_metadata['tlid_'+tltrack.tlid] !== undefined){
                    var metadata = core.queue_metadata['tlid_'+tltrack.tlid]
                } else {
                    var metadata = {}
                }

                var track = Object.assign(
                    {}, 
                    tltrack.track,
                    metadata,
                    { 
                        tlid: tltrack.tlid,
                        playing: (core.current_track && tltrack.tlid == core.current_track.tlid )
                    })
                tracklist.push(track)
            }

            tracklist = helpers.formatTracks(tracklist);

            return Object.assign({}, core, { current_tracklist: tracklist });

        case 'MOPIDY_CURRENTTLTRACK':
            if (!action.data) return core

            var current_tracklist = []
            Object.assign(current_tracklist, core.current_tracklist)

            for (var i = 0; i < current_tracklist.length; i++){
                Object.assign(
                    current_tracklist[i], 
                    { playing: (current_tracklist[i].tlid == action.data.tlid ) }
                )
            }

            var current_track = Object.assign(
                {},
                action.data.track,
                {
                    tlid: action.data.tlid
                }
            )

            return Object.assign({}, core, {
                current_tracklist: current_tracklist,
                current_track: current_track
            });

        case 'TRACK_LOADED':
            if (!action.key || !action.track) return core

            var tracks = Object.assign({}, core.tracks)
            if (tracks[action.key]){
                var track = Object.assign(
                    {}, 
                    tracks[action.key], 
                    helpers.formatTracks(action.track)
                );
            } else {
                var track = Object.assign(
                    {},
                    helpers.formatTracks(action.track)
                );
            }

            tracks[action.key] = track
            return Object.assign({}, core, { tracks: tracks });

        case 'TRACKS_LOADED':
            var tracks = Object.assign({}, core.tracks)

            for (var i = 0; i < action.tracks.length; i++){
                var track = action.tracks[i]
                if (tracks[track.uri] !== undefined){
                    track = Object.assign(
                        {}, 
                        tracks[track.uri], 
                        track
                    );
                }
                tracks[track.uri] = helpers.formatTracks(track);
            }

            return Object.assign({}, core, { tracks: tracks });

        case 'PUSHER_QUEUE_METADATA':
        case 'PUSHER_QUEUE_METADATA_CHANGED':
            var tracklist = Object.assign([], core.current_tracklist)
            for(var i = 0; i < tracklist.length; i++){

                // load our metadata (if we have any for that tlid)
                if (typeof(action.queue_metadata['tlid_'+tracklist[i].tlid]) !== 'undefined'){
                    tracklist[i] = Object.assign(
                        {},
                        tracklist[i],
                        action.queue_metadata['tlid_'+tracklist[i].tlid],
                    )
                }
            }
            return Object.assign({}, core, { current_tracklist: tracklist, queue_metadata: action.queue_metadata });

        case 'PUSHER_RADIO':
        case 'PUSHER_RADIO_STARTED':
        case 'PUSHER_RADIO_CHANGED':
        case 'PUSHER_RADIO_STOPPED':
            return Object.assign({}, core, { seeds_resolved: false }, { radio: action.radio })

        case 'RADIO_SEEDS_RESOLVED':
            var radio = Object.assign({}, core.radio, { resolved_seeds: action.resolved_seeds })
            return Object.assign({}, core, { radio: radio })




        /**
         * Categories
         **/

        case 'CATEGORY_LOADED':
            var categories = Object.assign([], core.categories)

            if (categories[action.key]){
                var category = Object.assign({}, categories[action.key], action.category)
            } else {
                var category = Object.assign({}, action.category)
            }

            categories[action.key] = category
            return Object.assign({}, core, { categories: categories });

        case 'CATEGORIES_LOADED':
            var categories = Object.assign([], core.categories)

            for (var i = 0; i < action.categories.length; i++){
                var key = 'category:'+action.categories[i].id
                if (categories[key]){
                    var category = Object.assign({}, categories[key], action.categories[i])
                } else {
                    var category = Object.assign({}, action.categories[i])
                }
                categories[key] = category
            }

            return Object.assign({}, core, { categories: categories });

        case 'CATEGORY_PLAYLISTS_LOADED':
            var categories = Object.assign([], core.categories)
            var playlists_uris = []
            if (categories[action.key].playlists_uris) playlists_uris = categories[action.key].playlists_uris

            var category = Object.assign(
                {}, 
                categories[action.key],
                {
                    playlists_uris: [...playlists_uris, ...action.uris],
                    playlists_more: action.more,
                    playlists_total: action.total
                }
            )
            categories[action.key] = category
            return Object.assign({}, core, { categories: categories });




        /**
         * Albums
         **/

        case 'ALBUM_LOADED':
            var albums = Object.assign([], core.albums)

            if (albums[action.key]){
                var album = Object.assign({}, albums[action.key], action.album)
            } else {
                var album = Object.assign({}, action.album)
            }

            album.tracks = helpers.formatTracks(album.tracks);
            albums[action.key] = album

            return Object.assign({}, core, { albums: albums });

        case 'ALBUMS_LOADED':
            var albums = Object.assign([], core.albums)

            for (var i = 0; i < action.albums.length; i++){
                var album = action.albums[i]
                if (albums[album.uri]){
                    album = Object.assign({}, albums[album.uri], album)
                }

                album.tracks = helpers.formatTracks(album.tracks);
                albums[album.uri] = album
            }

            return Object.assign({}, core, { albums: albums });

        case 'NEW_RELEASES_LOADED':
            if (!action.uris){
                return Object.assign({}, core, { 
                    new_releases: null,
                    new_releases_more: null,
                    new_releases_total: null
                });
            }

            var new_releases = []
            if (core.new_releases) new_releases = Object.assign([], core.new_releases)

            return Object.assign({}, core, { 
                new_releases: [...new_releases, ...action.uris],
                new_releases_more: action.more,
                new_releases_total: action.total
            });



        /**
         * Artists
         **/

        case 'ARTIST_LOADED':
            var artists = Object.assign([], core.artists)

            if (artists[action.key]){

                // if we've already got images, remove and add as additional_images
                // this is to prevent LastFM overwriting Spotify images
                if (artists[action.key].images){
                    action.artist.images_additional = action.artist.images
                    delete action.artist.images
                }

                var artist = Object.assign({}, artists[action.key], action.artist)
            } else {
                var artist = Object.assign({}, action.artist)
            }

            artists[action.key] = artist
            return Object.assign({}, core, { artists: artists });

        case 'ARTISTS_LOADED':
            var artists = Object.assign([], core.artists)

            for (var i = 0; i < action.artists.length; i++){
                var artist = action.artists[i]
                if (typeof(artists[artist.uri]) !== 'undefined'){
                    artist = Object.assign({}, artists[artist.uri], artist)
                }
                artists[artist.uri] = artist
            }

            return Object.assign({}, core, { artists: artists });

        case 'ARTIST_ALBUMS_LOADED':
            var artists = Object.assign([], core.artists)
            var albums_uris = []
            if (artists[action.key].albums_uris) albums_uris = artists[action.key].albums_uris

            var artist = Object.assign(
                {}, 
                artists[action.key],
                {
                    albums_uris: [...albums_uris, ...action.uris],
                    albums_more: action.more,
                    albums_total: action.total
                }
            )
            artists[action.key] = artist
            return Object.assign({}, core, { artists: artists });


        /**
         * User profiles
         **/

        case 'USER_LOADED':
            var users = Object.assign([], core.users)

            if (users[action.key]){
                var user = Object.assign({}, users[action.key], action.user)
            } else {
                var user = Object.assign({}, action.user)
            }

            users[action.key] = user
            return Object.assign({}, core, { users: users });

        case 'USER_PLAYLISTS_LOADED':
            var users = Object.assign([], core.users)
            var playlists_uris = []
            if (users[action.key] && users[action.key].playlists_uris) playlists_uris = users[action.key].playlists_uris

            var artist = Object.assign(
                {}, 
                users[action.key],
                {
                    playlists_uris: [...playlists_uris, ...action.uris],
                    playlists_more: action.more,
                    playlists_total: action.total
                }
            )
            users[action.key] = artist
            return Object.assign({}, core, { users: users });




        /**
         * Playlists
         **/

        case 'PLAYLIST_LOADED':
        case 'PLAYLIST_UPDATED':
            var playlists = Object.assign([], core.playlists)

            if (typeof(playlists[action.key]) !== 'undefined'){
                var existing_playlist = Object.assign({}, playlists[action.key])

                if (existing_playlist.tracks && action.playlist.tracks){
                    var tracks = [...existing_playlist.tracks, ...action.playlist.tracks]
                } else if (existing_playlist.tracks){
                    var tracks = existing_playlist.tracks
                } else if (action.playlist.tracks){
                    var tracks = action.playlist.tracks
                } else {
                    var tracks = []
                }

                var merged_playlist = Object.assign(
                    {},
                    existing_playlist, 
                    action.playlist,
                    {
                        tracks: helpers.formatTracks(tracks)
                    }
                )
            } else {
                var merged_playlist = Object.assign({}, action.playlist)
            }

            playlists[action.key] = merged_playlist
            return Object.assign({}, core, { playlists: playlists })

        case 'PLAYLIST_KEY_UPDATED':
            var playlists = Object.assign([], core.playlists)

            // URI not in our index? No change needed then
            if (typeof(playlists[action.key]) === 'undefined'){
                return core
            }

            // Delete our old playlist by key, and add by new key
            var playlist = Object.assign({}, playlists[action.key])
            delete playlists[action.key]
            playlists[playlist.uri] = playlist

            return Object.assign({}, core, { playlists: playlists })

        case 'PLAYLISTS_LOADED':
            var playlists = Object.assign([], core.playlists)

            for (var i = 0; i < action.playlists.length; i++){
                var loaded_playlist = action.playlists[i]

                if (typeof(playlists[loaded_playlist.uri]) !== 'undefined'){
                    var existing_playlist = Object.assign({}, playlists[loaded_playlist.uri])

                    if (existing_playlist.tracks && loaded_playlist.tracks){
                        var tracks = [...existing_playlist.tracks, ...loaded_playlist.tracks]
                    } else if (existing_playlist.tracks){
                        var tracks = existing_playlist.tracks
                    } else if (loaded_playlist.tracks){
                        var tracks = loaded_playlist.tracks
                    } else {
                        var tracks = []
                    }

                    var merged_playlist = Object.assign(
                        {}, 
                        existing_playlist,
                        loaded_playlist,
                        {
                            tracks: helpers.formatTracks(tracks)
                        }
                    )

                } else {
                    var merged_playlist = loaded_playlist
                }

                playlists[merged_playlist.uri] = merged_playlist
            }

            return Object.assign({}, core, { playlists: playlists });

        case 'PLAYLIST_LOADED_MORE_TRACKS':
            var playlists = Object.assign([], core.playlists)
            var playlist = Object.assign(
                {}, 
                playlists[action.key],
                {
                    tracks: [...playlists[action.key].tracks, ...helpers.formatTracks(action.data.items)],
                    tracks_more: action.data.next,
                    tracks_total: action.data.total
                }
            )

            playlists[action.key] = playlist
            return Object.assign({}, core, { playlists: playlists });

        case 'PLAYLIST_TRACKS_REMOVED':
            var playlists = Object.assign([], core.playlists)
            var playlist = Object.assign({}, playlists[action.key])
            var tracks = Object.assign([], playlist.tracks)
            var indexes = action.tracks_indexes.reverse()
            for(var i = 0; i < indexes.length; i++){
                tracks.splice(indexes[i], 1 )
            }
            var snapshot_id = null
            if (action.snapshot_id ) snapshot_id = action.snapshot_id
            Object.assign(playlist, { tracks: tracks, snapshot_id: snapshot_id })
            playlists[action.key] = playlist
            return Object.assign({}, core, { playlists: playlists });

        case 'PLAYLIST_TRACKS':
            var playlists = Object.assign([], core.playlists)
            var playlist = Object.assign({}, playlists[action.key], { tracks: helpers.formatTracks(action.tracks) })

            playlists[action.key] = playlist
            return Object.assign({}, core, { playlists: playlists });

        case 'PLAYLIST_TRACKS_REORDERED':
            var playlists = Object.assign([], core.playlists)
            var playlist = Object.assign({}, playlists[action.key])
            var tracks = Object.assign([], playlist.tracks)

            // handle insert_before offset if we're moving BENEATH where we're slicing tracks
            var insert_before = action.insert_before
            if (insert_before > action.range_start ) insert_before = insert_before - action.range_length

            // cut our moved tracks into a new array
            var tracks_to_move = tracks.splice(action.range_start, action.range_length)
            tracks_to_move.reverse()

            for(i = 0; i < tracks_to_move.length; i++){
                tracks.splice(insert_before, 0, tracks_to_move[i])
            }

            var snapshot_id = null
            if (action.snapshot_id ) snapshot_id = action.snapshot_id
            Object.assign(playlist, { tracks: tracks, snapshot_id: snapshot_id })
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





import * as helpers from '../../helpers'

export default function reducer(ui = {}, action){
    switch (action.type) {

        case 'LAZY_LOADING':
            return Object.assign({}, ui, { lazy_loading: action.start });

        case 'DEBUG':
            return Object.assign({}, ui, { debug_response: action.response })

        case 'UI_SET':
            return Object.assign({}, ui, action.data)

        case 'PUSHER_CONFIG':
            return Object.assign({}, ui, { config: action.config })

        case 'TOGGLE_SIDEBAR':
            var new_state = !ui.sidebar_open
            if( typeof(action.new_state) !== 'undefined' ) new_state = action.new_state
            return Object.assign({}, ui, { sidebar_open : new_state })


        /**
         * Context menu
         **/

        case 'SHOW_CONTEXT_MENU':
            return Object.assign({}, ui, { 
                context_menu: action.data
            });

        case 'HIDE_CONTEXT_MENU':
            return Object.assign({}, ui, {context_menu: null});

        case 'SHOW_TOUCH_CONTEXT_MENU':
            return Object.assign({}, ui, { 
            	touch_context_menu: action.data
            });

        case 'HIDE_TOUCH_CONTEXT_MENU':
            return Object.assign({}, ui, {touch_context_menu: null});



        /**
         * Dragging
         **/

        case 'DRAG_START':
            return Object.assign({}, ui, { 
                dragger: { 
                    dragging: true,
                    active: false,
                    context: action.context, 
                    from_uri: action.from_uri, 
                    victims: action.victims,
                    victims_indexes: action.victims_indexes,
                    start_x: action.start_x,
                    start_y: action.start_y
                }
            });

        case 'DRAG_ACTIVE':
            var dragger = Object.assign({}, ui.dragger, { active: true })
            return Object.assign({}, ui, { dragger: dragger });

        case 'DRAG_END':
            return Object.assign({}, ui, { 
                dragger: false
            });



        /**
         * Current track and tracklist
         **/

        case 'MOPIDY_TLTRACKS':
            if( !action.data ) return ui

            var tracklist = []
            for( var i = 0; i < action.data.length; i++ ){

                var tltrack = action.data[i]

                // load our metadata (if we have any for that tlid)
                if (typeof(ui.queue_metadata) !== 'undefined' && typeof(ui.queue_metadata['tlid_'+tltrack.tlid]) !== 'undefined'){
                    var metadata = ui.queue_metadata['tlid_'+tltrack.tlid]
                } else {
                    var metadata = {}
                }

                var track = Object.assign(
                    {}, 
                    tltrack.track,
                    metadata,
                    { 
                        tlid: tltrack.tlid,
                        playing: ( ui.current_track && tltrack.tlid == ui.current_track.tlid )
                    })
                tracklist.push( track )
            }

            return Object.assign({}, ui, { current_tracklist: tracklist });

        case 'MOPIDY_CURRENTTLTRACK':
            if (!action.data) return ui

            var current_tracklist = []
            Object.assign(current_tracklist, ui.current_tracklist)

            for (var i = 0; i < current_tracklist.length; i++){
                Object.assign(
                    current_tracklist[i], 
                    { playing: ( current_tracklist[i].tlid == action.data.tlid ) }
                )
            }

            var current_track = Object.assign(
                {},
                action.data.track,
                {
                    tlid: action.data.tlid
                }
            )

            return Object.assign({}, ui, {
                current_tracklist: current_tracklist,
                current_track: current_track
            });

        case 'TRACK_LOADED':
            if (!action.key || !action.track) return ui

            var tracks = Object.assign({}, ui.tracks)
            if (tracks[action.key]){
                var track = Object.assign({}, tracks[action.key], action.track)
            }else{
                var track = Object.assign({}, action.track)
            }

            tracks[action.key] = track
            return Object.assign({}, ui, { tracks: tracks });

        case 'TRACKS_LOADED':
            var tracks = Object.assign({}, ui.tracks)

            for (var i = 0; i < action.tracks.length; i++){
                var track = action.tracks[i]
                if (typeof(tracks[track.uri]) !== 'undefined'){
                    track = Object.assign({}, tracks[track.uri], track)
                }
                tracks[track.uri] = track
            }

            return Object.assign({}, ui, { tracks: tracks });

        case 'PUSHER_QUEUE_METADATA':
        case 'PUSHER_QUEUE_METADATA_CHANGED':
            var tracklist = Object.assign([], ui.current_tracklist)
            for( var i = 0; i < tracklist.length; i++ ){

                // load our metadata (if we have any for that tlid)
                if (typeof(action.queue_metadata['tlid_'+tracklist[i].tlid]) !== 'undefined'){
                    tracklist[i] = Object.assign(
                        {},
                        tracklist[i],
                        action.queue_metadata['tlid_'+tracklist[i].tlid],
                    )
                }
            }
            return Object.assign({}, ui, { current_tracklist: tracklist, queue_metadata: action.queue_metadata });

        case 'PUSHER_RADIO':
        case 'PUSHER_RADIO_STARTED':
        case 'PUSHER_RADIO_CHANGED':
        case 'PUSHER_RADIO_STOPPED':
            return Object.assign({}, ui, { seeds_resolved: false }, { radio: action.radio })

        case 'RADIO_SEEDS_RESOLVED':
            var radio = Object.assign({}, ui.radio, { resolved_seeds: action.resolved_seeds })
            return Object.assign({}, ui, { radio: radio })




        /**
         * Categories
         **/

        case 'CATEGORY_LOADED':
            var categories = Object.assign([], ui.categories)

            if (categories[action.key]){
                var category = Object.assign({}, categories[action.key], action.category)
            }else{
                var category = Object.assign({}, action.category)
            }

            categories[action.key] = category
            return Object.assign({}, ui, { categories: categories });

        case 'CATEGORIES_LOADED':
            var categories = Object.assign([], ui.categories)

            for (var i = 0; i < action.categories.length; i++){
                var key = 'category:'+action.categories[i].id
                if (categories[key]){
                    var category = Object.assign({}, categories[key], action.categories[i])
                }else{
                    var category = Object.assign({}, action.categories[i])
                }
                categories[key] = category
            }

            return Object.assign({}, ui, { categories: categories });

        case 'CATEGORY_PLAYLISTS_LOADED':
            var categories = Object.assign([], ui.categories)
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
            return Object.assign({}, ui, { categories: categories });




        /**
         * Albums
         **/

        case 'ALBUM_LOADED':
            var albums = Object.assign([], ui.albums)

            if (albums[action.key]){
                var album = Object.assign({}, albums[action.key], action.album)
            }else{
                var album = Object.assign({}, action.album)
            }

            albums[action.key] = album
            return Object.assign({}, ui, { albums: albums });

        case 'ALBUMS_LOADED':
            var albums = Object.assign([], ui.albums)

            for (var i = 0; i < action.albums.length; i++){
                var album = action.albums[i]
                if (albums[album.uri]){
                    album = Object.assign({}, albums[album.uri], album)
                }
                albums[album.uri] = album
            }

            return Object.assign({}, ui, { albums: albums });

        case 'LIBRARY_ALBUMS_LOADED':
            var library_albums = []
            if (ui.library_albums) library_albums = Object.assign([], ui.library_albums)

            return Object.assign({}, ui, { 
                library_albums: helpers.removeDuplicates([...library_albums, ...action.uris]),
                library_albums_more: action.more,
                library_albums_total: action.total,
                library_albums_started: true
            });

        case 'ALBUM_LIBRARY_CHECK':
            var items = Object.assign([], ui.library_albums)

            // add/remove library reference
            var index = items.indexOf(action.key)

            // removing existing
            if (index > -1 && !action.in_library){
                items.splice(index, 1)
            } else if (index < 0 && action.in_library){
                items.push(action.key)
            }

            return Object.assign({}, ui, { library_albums: items });

        case 'LOCAL_ALBUMS_LOADED':
            if (!action.uris) return Object.assign({}, ui, { local_albums: null });
            return Object.assign({}, ui, { local_albums: action.uris });

        case 'NEW_RELEASES_LOADED':
            if (!action.uris){
                return Object.assign({}, ui, { 
                    new_releases: null,
                    new_releases_more: null,
                    new_releases_total: null
                });
            }

            var new_releases = []
            if (ui.new_releases) new_releases = Object.assign([], ui.new_releases)

            return Object.assign({}, ui, { 
                new_releases: [...new_releases, ...action.uris],
                new_releases_more: action.more,
                new_releases_total: action.total
            });



        /**
         * Artists
         **/

        case 'ARTIST_LOADED':
            var artists = Object.assign([], ui.artists)

            if (artists[action.key]){

                // if we've already got images, remove and add as additional_images
                // this is to prevent LastFM overwriting Spotify images
                if (artists[action.key].images){
                    action.artist.images_additional = action.artist.images
                    delete action.artist.images
                }

                var artist = Object.assign({}, artists[action.key], action.artist)
            }else{
                var artist = Object.assign({}, action.artist)
            }

            artists[action.key] = artist
            return Object.assign({}, ui, { artists: artists });

        case 'ARTISTS_LOADED':
            var artists = Object.assign([], ui.artists)

            for (var i = 0; i < action.artists.length; i++){
                var artist = action.artists[i]
                if (typeof(artists[artist.uri]) !== 'undefined'){
                    artist = Object.assign({}, artists[artist.uri], artist)
                }
                artists[artist.uri] = artist
            }

            return Object.assign({}, ui, { artists: artists });

        case 'ARTIST_ALBUMS_LOADED':
            var artists = Object.assign([], ui.artists)
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
            return Object.assign({}, ui, { artists: artists });

        case 'LIBRARY_ARTISTS_LOADED':
            var library_artists = []
            if (ui.library_artists) library_artists = Object.assign([], ui.library_artists)

            return Object.assign({}, ui, { 
                library_artists: helpers.removeDuplicates([...library_artists, ...action.uris]),
                library_artists_more: action.more,
                library_artists_total: action.total,
                library_artists_started: true
            });

        case 'ARTIST_LIBRARY_CHECK':
            var items = Object.assign([], ui.library_artists)

            // add/remove library reference
            var index = items.indexOf(action.key)

            // removing existing
            if (index > -1 && !action.in_library){
                items.splice(index, 1)
            } else if (index < 0 && action.in_library){
                items.push(action.key)
            }

            return Object.assign({}, ui, { library_artists: items });

        case 'LOCAL_ARTISTS_LOADED':
            if (!action.uris) return Object.assign({}, ui, { local_artists: null });
            return Object.assign({}, ui, { local_artists: action.uris });


        /**
         * User profiles
         **/

        case 'USER_LOADED':
            var users = Object.assign([], ui.users)

            if (users[action.key]){
                var user = Object.assign({}, users[action.key], action.user)
            }else{
                var user = Object.assign({}, action.user)
            }

            users[action.key] = user
            return Object.assign({}, ui, { users: users });

        case 'USER_PLAYLISTS_LOADED':
            var users = Object.assign([], ui.users)
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
            return Object.assign({}, ui, { users: users });




        /**
         * Playlists
         **/

        case 'PLAYLIST_LOADED':
        case 'PLAYLIST_UPDATED':
            var playlists = Object.assign([], ui.playlists)

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
                        tracks: tracks
                    }
                )
            }else{
                var merged_playlist = Object.assign({}, action.playlist)
            }

            playlists[action.key] = merged_playlist
            return Object.assign({}, ui, { playlists: playlists });

        case 'PLAYLISTS_LOADED':
            var playlists = Object.assign([], ui.playlists)

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
                            tracks: tracks
                        }
                    )

                } else {
                    var merged_playlist = loaded_playlist
                }

                playlists[merged_playlist.uri] = merged_playlist
            }

            return Object.assign({}, ui, { playlists: playlists });

        case 'PLAYLIST_LOADED_MORE_TRACKS':
            var playlists = Object.assign([], ui.playlists)
            var playlist = Object.assign(
                {}, 
                playlists[action.key],
                {
                    tracks: [...playlists[action.key].tracks, ...helpers.flattenTracks(action.data.items)],
                    tracks_more: action.data.next,
                    tracks_total: action.data.total
                }
            )

            playlists[action.key] = playlist
            return Object.assign({}, ui, { playlists: playlists });

        case 'PLAYLIST_TRACKS_REMOVED':
            var playlists = Object.assign([], ui.playlists)
            var playlist = Object.assign({}, playlists[action.key])
            var tracks = Object.assign([], playlist.tracks)
            var indexes = action.tracks_indexes.reverse()
            for( var i = 0; i < indexes.length; i++ ){
                tracks.splice( indexes[i], 1 )
            }
            var snapshot_id = null
            if( action.snapshot_id ) snapshot_id = action.snapshot_id
            Object.assign(playlist, { tracks: tracks, snapshot_id: snapshot_id })
            playlists[action.key] = playlist
            return Object.assign({}, ui, { playlists: playlists });

        case 'PLAYLIST_TRACKS':
            var playlists = Object.assign([], ui.playlists)
            var playlist = Object.assign({}, playlists[action.key], { tracks: action.tracks })

            playlists[action.key] = playlist
            return Object.assign({}, ui, { playlists: playlists });

        case 'PLAYLIST_TRACKS_REORDERED':
            var playlists = Object.assign([], ui.playlists)
            var playlist = Object.assign({}, playlists[action.key])
            var tracks = Object.assign([], playlist.tracks)

            // handle insert_before offset if we're moving BENEATH where we're slicing tracks
            var insert_before = action.insert_before
            if( insert_before > action.range_start ) insert_before = insert_before - action.range_length

            // cut our moved tracks into a new array
            var tracks_to_move = tracks.splice(action.range_start, action.range_length)
            tracks_to_move.reverse()

            for( i = 0; i < tracks_to_move.length; i++ ){
                tracks.splice(insert_before, 0, tracks_to_move[i])
            }

            var snapshot_id = null
            if( action.snapshot_id ) snapshot_id = action.snapshot_id
            Object.assign(playlist, { tracks: tracks, snapshot_id: snapshot_id })
            playlists[action.key] = playlist
            return Object.assign({}, ui, { playlists: playlists });

        case 'LIBRARY_PLAYLISTS_LOADED':
            if (ui.library_playlists){
                var library_playlists = [...ui.library_playlists, ...action.uris]
            }else{
                var library_playlists = action.uris
            }

            library_playlists = helpers.removeDuplicates(library_playlists)

            return Object.assign({}, ui, { 
                library_playlists: library_playlists,
                library_playlists_started: true
            });

        case 'PLAYLIST_LIBRARY_CHECK':
            var items = Object.assign([], ui.library_playlists)

            // add/remove library reference
            var index = items.indexOf(action.key)

            // removing existing
            if (index > -1 && !action.in_library){
                items.splice(index, 1)
            } else if (index < 0 && action.in_library){
                items.push(action.key)
            }

            return Object.assign({}, ui, { library_playlists: items });


        /**
         * Genres
         **/

        case 'SPOTIFY_GENRES_LOADED':
            return Object.assign({}, ui, {genres: action.genres})


        /**
         * Search results
         **/

        case 'SEARCH_STARTED':
            return Object.assign({}, ui, {
                search_results: {
                    artists_more: null,
                    artists_uris: [],
                    albums_more: null,
                    albums_uris: [],
                    playlists_more: null,
                    playlists_uris: [],
                    tracks: [],
                    tracks_more: null,
                }
            });

        case 'SEARCH_RESULTS_LOADED':

            // artists
            if (ui.search_results && ui.search_results.artists_uris){
                var artists_uris = ui.search_results.artists_uris
            }else{
                var artists_uris = []
            }
            if (action.artists_uris) artists_uris = [...artists_uris, ...action.artists_uris]

            // more tracks
            if (typeof(action.artists_more) !== 'undefined') var artists_more = action.artists_more
            else if (ui.search_results && ui.search_results.artists_more) var artists_more = ui.search_results.artists_more
            else var artists_more = null


            // albums
            if (ui.search_results && ui.search_results.albums_uris){
                var albums_uris = ui.search_results.albums_uris
            }else{
                var albums_uris = []
            }
            if (action.albums_uris) albums_uris = [...albums_uris, ...action.albums_uris]

            // more tracks
            if (typeof(action.albums_more) !== 'undefined') var albums_more = action.albums_more
            else if (ui.search_results && ui.search_results.albums_more) var albums_more = ui.search_results.albums_more
            else var albums_more = null


            // playlists
            if (ui.search_results && ui.search_results.playlists_uris){
                var playlists_uris = ui.search_results.playlists_uris
            }else{
                var playlists_uris = []
            }
            if (action.playlists_uris) playlists_uris = [...playlists_uris, ...action.playlists_uris]

            // more tracks
            if (typeof(action.playlists_more) !== 'undefined') var playlists_more = action.playlists_more
            else if (ui.search_results && ui.search_results.playlists_more) var playlists_more = ui.search_results.playlists_more
            else var playlists_more = null


            // tracks
            if (ui.search_results && ui.search_results.tracks){
                var tracks = ui.search_results.tracks
            }else{
                var tracks = []
            }
            if (action.tracks) tracks = [...tracks, ...action.tracks]

            // more tracks
            if (typeof(action.tracks_more) !== 'undefined') var tracks_more = action.tracks_more
            else if (ui.search_results && ui.search_results.tracks_more) var tracks_more = ui.search_results.tracks_more
            else var tracks_more = null

            return Object.assign({}, ui, {
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


        /**
         * Modals
         **/

        case 'OPEN_MODAL':
            return Object.assign({}, ui, { modal: action.modal })

        case 'CLOSE_MODAL':
            return Object.assign({}, ui, { modal: false })


        /**
         * Notifications
         **/

        case 'CREATE_NOTIFICATION':
            var notifications = [...ui.notifications, action.notification]
            notifications = helpers.mergeDuplicates(notifications,'key')
            return Object.assign({}, ui, { notifications: notifications })

        case 'REMOVE_NOTIFICATION':
            var notifications = Object.assign([], ui.notifications)
            
            if( action.index > -1 ){
                notifications.splice(action.index, 1)
            }

            return Object.assign({}, ui, {notifications: notifications})




        /**
         * Loading and processes
         **/

         case 'START_LOADING':
            var load_queue = Object.assign({}, (ui.load_queue ? ui.load_queue : {}))
            load_queue[action.key] = action.source
            return Object.assign({}, ui, {load_queue: load_queue})

         case 'STOP_LOADING':
            var load_queue = Object.assign({}, (ui.load_queue ? ui.load_queue : {}))
            if (load_queue[action.key]){
                delete load_queue[action.key]
            }
            return Object.assign({}, ui, {load_queue: load_queue})

         case 'START_PROCESS':
            var processes = Object.assign({}, (ui.processes ? ui.processes : []))
            processes[action.key] = {
                key: action.key,
                content: action.content
            }
            return Object.assign({}, ui, {processes: processes})

         case 'CANCEL_PROCESS':
            var processes = Object.assign({}, (ui.processes ? ui.processes : {}))
            if (processes[action.key]){
                processes[action.key] = Object.assign(
                    {},
                    processes[action.key],
                    {
                        cancelling: true
                    }
                )
            }
            return Object.assign({}, ui, {processes: processes})

         case 'STOP_PROCESS':
            var processes = Object.assign({}, (ui.processes ? ui.processes : {}))
            if (processes[action.key]){
                delete processes[action.key]
            }
            return Object.assign({}, ui, {processes: processes})


        default:
            return ui
    }
}





import * as helpers from '../../helpers'

export default function reducer(ui = {}, action){
    switch (action.type) {

        case 'LAZY_LOADING':
            return Object.assign({}, ui, { lazy_loading: action.start });

        case 'DEBUG':
            return Object.assign({}, ui, { debug_response: action.response })

        case 'UI_SET':
            return Object.assign({}, ui, action.data)

        case 'TOGGLE_SIDEBAR':
            var new_state = !ui.sidebar_open
            if( typeof(action.new_state) !== 'undefined' ) new_state = action.new_state
            return Object.assign({}, ui, { sidebar_open : new_state })


        /**
         * Context menu
         **/
        case 'SHOW_CONTEXT_MENU':
            return Object.assign({}, ui, { 
            	context_menu: { 
            		show: true,
            		position_x: action.position_x,
            		position_y: action.position_y,
            		context: action.context, 
                    trigger: action.trigger,
            		data: action.data
            	}
            });

        case 'HIDE_CONTEXT_MENU':
            return Object.assign({}, ui, { context_menu: { show: false } });



        /**
         * Dragging
         **/

        case 'DRAG_START':
            return Object.assign({}, ui, { 
                dragger: { 
                    dragging: true,
                    active: false,
                    context: action.context, 
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
                var track = Object.assign(
                    {}, 
                    action.data[i].track, 
                    { 
                        tlid: action.data[i].tlid,
                        playing: ( ui.current_track && action.data[i].tlid == ui.current_track.tlid )
                    })
                tracklist.push( track )
            }

            return Object.assign({}, ui, { current_tracklist: tracklist });

        case 'SPOTIFY_TRACK_LOADED':
            if( !action.data ) return ui
                
            var current_track = Object.assign({}, ui.current_track, action.data)
            return Object.assign({}, ui, { current_track: current_track });

        case 'MOPIDY_CURRENTTLTRACK':
            if( !action.data ) return ui

            var current_tracklist = []
            Object.assign(current_tracklist, ui.current_tracklist)

            for( var i = 0; i < current_tracklist.length; i++ ){
                Object.assign(
                    current_tracklist[i], 
                    { playing: ( current_tracklist[i].tlid == action.data.tlid ) }
                )
            }

            var current_track = action.data.track
            Object.assign(current_track, { tlid: action.data.tlid })

            return Object.assign({}, ui, {
                current_track: current_track
            });

        case 'RADIO':
        case 'START_RADIO':
            return Object.assign({}, ui, { seeds_resolved: false }, { radio: action.data.radio })

        case 'RADIO_SEEDS_RESOLVED':
            var radio = Object.assign({}, ui.radio, action.data, { seeds_resolved: true })
            return Object.assign({}, ui, { radio: radio })




        /**
         * Albums
         **/

        case 'ALBUM_LOADED':
            var albums = Object.assign([], ui.albums)

            if (albums[action.uri]){
                var album = Object.assign({}, albums[action.uri], action.album)
            }else{
                var album = Object.assign({}, action.album)
            }

            albums[action.uri] = album
            return Object.assign({}, ui, { albums: albums });

        case 'ALBUMS_LOADED':
            var albums = Object.assign([], ui.albums)

            for (var i = 0; i < action.albums.length; i++){
                var album = action.albums[i]
                if (typeof(albums[album.uri]) !== 'undefined'){
                    artist = Object.assign({}, albums[album.uri], album)
                }
                albums[album.uri] = album
            }

            return Object.assign({}, ui, { albums: albums });

        case 'LIBRARY_ALBUMS_LOADED':
            if (!action.uris){
                return Object.assign({}, ui, { 
                    library_albums: null,
                    library_albums_more: null,
                    library_albums_total: null
                });
            }

            var library_albums = []
            if (ui.library_albums) library_albums = Object.assign([], ui.library_albums)

            return Object.assign({}, ui, { 
                library_albums: [...library_albums, ...action.uris],
                library_albums_more: action.more,
                library_albums_total: action.total
            });

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

            if (artists[action.uri]){
                // if we've already got images, delete our new ones
                // this is to prevent LastFM overwriting Spotify images
                if (artists[action.uri].images) delete action.artist.images
                var artist = Object.assign({}, artists[action.uri], action.artist)
            }else{
                var artist = Object.assign({}, action.artist)
            }

            artists[action.uri] = artist
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
            if (artists[action.uri].albums_uris) albums_uris = artists[action.uri].albums_uris

            var artist = Object.assign(
                {}, 
                artists[action.uri],
                {
                    albums_uris: [...albums_uris, ...action.uris],
                    albums_more: action.more,
                    albums_total: action.total
                }
            )
            artists[action.uri] = artist
            return Object.assign({}, ui, { artists: artists });

        case 'LIBRARY_ARTISTS_LOADED':
            if (!action.uris){
                return Object.assign({}, ui, { 
                    library_artists: null,
                    library_artists_more: null,
                    library_artists_total: null
                });
            }

            var library_artists = []
            if (ui.library_artists) library_artists = Object.assign([], ui.library_artists)

            return Object.assign({}, ui, { 
                library_artists: [...library_artists, ...action.uris],
                library_artists_more: action.more,
                library_artists_total: action.total
            });

        case 'LOCAL_ARTISTS_LOADED':
            if (!action.uris) return Object.assign({}, ui, { local_artists: null });
            return Object.assign({}, ui, { local_artists: action.uris });


        /**
         * User
         **/

        case 'SPOTIFY_USER_LOADED':
            if( !action.data ) return Object.assign({}, ui, { user: false })
            return Object.assign({}, ui, { user: action.data })

        case 'SPOTIFY_USER_PLAYLISTS_LOADED_MORE':
            var user = Object.assign({}, ui.user, {
                playlists: [ ...ui.user.playlists, ...action.data.items ],
                playlists_more: action.data.next
            })
            return Object.assign({}, ui, { user: user });

        case 'USER_FOLLOWING_LOADED':
            var user = Object.assign({}, ui.user, { following: action.is_following })
            return Object.assign({}, ui, { user: user, following_loading: false });


        /**
         * Playlists
         **/

        case 'PLAYLIST_LOADED':
        case 'PLAYLIST_UPDATED':
            var playlists = Object.assign([], ui.playlists)

            if (playlists[action.uri]){
                var playlist = Object.assign({}, playlists[action.uri], action.playlist)
            }else{
                var playlist = Object.assign({}, action.playlist)
            }

            playlists[action.uri] = playlist
            return Object.assign({}, ui, { playlists: playlists });

        case 'PLAYLISTS_LOADED':
            var playlists = Object.assign([], ui.playlists)

            for (var i = 0; i < action.playlists.length; i++){
                var playlist = action.playlists[i]
                if (typeof(playlists[playlist.uri]) !== 'undefined'){
                    artist = Object.assign({}, playlists[playlist.uri], playlist)
                }
                playlists[playlist.uri] = playlist
            }

            return Object.assign({}, ui, { playlists: playlists });

        case 'MOPIDY_PLAYLIST_LOADED':
            if( !action.data ) return Object.assign({}, ui, { playlist: false })
            return Object.assign({}, ui, { playlist: action.data })

        case 'PLAYLIST_LOADED_MORE_TRACKS':
            var playlists = Object.assign([], ui.playlists)
            var playlist = Object.assign(
                {}, 
                playlists[action.uri],
                {
                    tracks: [...playlists[action.uri].tracks, ...helpers.flattenTracks(action.data.items)],
                    tracks_more: action.data.next,
                    tracks_total: action.data.total
                }
            )

            playlists[action.uri] = playlist
            return Object.assign({}, ui, { playlists: playlists });

        case 'PLAYLIST_TRACKS_REMOVED':
            var playlists = Object.assign([], ui.playlists)
            var playlist = Object.assign({}, playlists[action.uri])
            var tracks = Object.assign([], playlist.tracks)
            var indexes = action.tracks_indexes.reverse()
            for( var i = 0; i < indexes.length; i++ ){
                tracks.splice( indexes[i], 1 )
            }
            var snapshot_id = null
            if( action.snapshot_id ) snapshot_id = action.snapshot_id
            Object.assign(playlist, { tracks: tracks, snapshot_id: snapshot_id })
            playlists[action.uri] = playlist
            return Object.assign({}, ui, { playlists: playlists });

        case 'PLAYLIST_TRACKS':
            var playlists = Object.assign([], ui.playlists)
            var playlist = Object.assign({}, playlists[action.uri], { tracks: action.tracks })

            playlists[action.uri] = playlist
            return Object.assign({}, ui, { playlists: playlists });

        case 'PLAYLIST_TRACKS_REORDERED':
            var playlists = Object.assign([], ui.playlists)
            var playlist = Object.assign({}, playlists[action.uri])
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
            playlists[action.uri] = playlist
            return Object.assign({}, ui, { playlists: playlists });

        case 'PLAYLIST_FOLLOWING_LOADED':
            var playlists = Object.assign([], ui.playlists)
            var playlist = Object.assign({}, playlists[action.uri], { following: action.is_following })

            playlists[action.uri] = playlist
            return Object.assign({}, ui, { playlists: playlists });

        case 'LIBRARY_PLAYLISTS_LOADED':
            if (ui.library_playlists){
                var library_playlists = [...ui.library_playlists, ...action.uris]
            }else{
                var library_playlists = action.uris
            }

            library_playlists = helpers.removeDuplicates(library_playlists)

            return Object.assign({}, ui, { 
                library_playlists: library_playlists
            });


        /**
         * Search results
         **/

        case 'SEARCH_RESULTS_LOADED':
            console.log(action)

            // artists
            if (ui.search_results && ui.search_results.artists_uris){
                var artists_uris = ui.search_results.artists_uris
            }else{
                var artists_uris = []
            }
            if (action.artists_uris) artists_uris = [...artists_uris, ...action.artists_uris]

            // albums
            if (ui.search_results && ui.search_results.albums_uris){
                var albums_uris = ui.search_results.albums_uris
            }else{
                var albums_uris = []
            }
            if (action.albums_uris) albums_uris = [...albums_uris, ...action.albums_uris]

            // playlists
            if (ui.search_results && ui.search_results.playlists_uris){
                var playlists_uris = ui.search_results.playlists_uris
            }else{
                var playlists_uris = []
            }
            if (action.playlists_uris) playlists_uris = [...playlists_uris, ...action.playlists_uris]

            // tracks
            if (ui.search_results && ui.search_results.tracks){
                var tracks = ui.search_results.tracks
            }else{
                var tracks = []
            }
            if (action.tracks) tracks = [...tracks, ...action.tracks]

            return Object.assign({}, ui, {
                search_results: {
                    artists_more: (action.artists_more ? action.artists_more : null),
                    artists_uris: artists_uris,
                    albums_more: (action.albums_more ? action.albums_more : null),
                    albums_uris: albums_uris,
                    playlists_more: (action.playlists_more ? action.playlists_more : null),
                    playlists_uris: playlists_uris,
                    tracks: tracks,
                    tracks_more: (action.tracks_more ? action.tracks.tracks_more : null)
                }
            });

        case 'SPOTIFY_SEARCH_RESULTS_LOADED_MORE_ARTISTS':
            var results = Object.assign(
                {}, 
                ui.search_results, 
                { 
                    artists_uris: [...ui.search_results.artists_uris, ...action.data.artists_uris],
                    artists_more: action.data.artists.next
                }
            )
            return Object.assign({}, ui, { search_results: results })

        case 'SPOTIFY_SEARCH_RESULTS_LOADED_MORE_ALBUMS':
            var albums = [...ui.search_results.albums, ...action.data.albums.items]
            var results = Object.assign({}, ui.search_results, { 
                albums: albums,
                albums_more: action.data.albums.next
            })
            return Object.assign({}, ui, { search_results: results })

        case 'SPOTIFY_SEARCH_RESULTS_LOADED_MORE_PLAYLISTS':
            var playlists = [...ui.search_results.playlists, ...action.data.playlists.items]
            var results = Object.assign({}, ui.search_results, { 
                playlists: playlists,
                playlists_more: action.data.playlists.next
            })
            return Object.assign({}, ui, { search_results: results })

        case 'SPOTIFY_SEARCH_RESULTS_LOADED_MORE_TRACKS':
            var tracks = [...ui.search_results.tracks, ...action.data.tracks.items]
            var results = Object.assign({}, ui.search_results, { 
                tracks: tracks,
                tracks_more: action.data.tracks.next
            })
            return Object.assign({}, ui, { search_results: results })


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
            return Object.assign({}, ui, { notifications: notifications })

        case 'REMOVE_NOTIFICATION':
            var notifications = Object.assign([], ui.notifications)

            function getByID( notification ){
                return notification.id === action.id
            }
            var index = notifications.findIndex(getByID)
            if( index > -1 ) notifications.splice(index, 1)

            return Object.assign({}, ui, { notifications: notifications })


        default:
            return ui
    }
}




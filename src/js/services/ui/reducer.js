
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

        case 'MOPIDY_ALBUM_LOADED':
            if( !action.data ) return Object.assign({}, ui, { album: false })
            return Object.assign({}, ui, { album: action.data });

        case 'LASTFM_ALBUM_LOADED':
            if( !action.data.image ) return ui

            var album = Object.assign({}, ui.album, { images: action.data.image })
            return Object.assign({}, ui, { album: album });

        case 'SPOTIFY_ALBUM_LOADED':
            if( !action.data ) return Object.assign({}, ui, { album: false })

            var album = Object.assign({}, { images: [] }, action.data, {
                tracks: action.data.tracks.items,
                tracks_total: action.data.tracks.total,
                tracks_more: action.data.tracks.next
            })
            return Object.assign({}, ui, { album: album })

        case 'SPOTIFY_ALBUM_LOADED_MORE':
            var album = Object.assign({}, ui.album, {
                tracks: [ ...ui.album.tracks, ...action.data.items ],
                tracks_more: action.data.next
            })
            return Object.assign({}, ui, { album: album });

        case 'ALBUM_FOLLOWING_LOADED':
            var album = Object.assign({}, ui.album, { following: action.is_following })
            return Object.assign({}, ui, { album: album, following_loading: false });



        /**
         * Artists
         **/

        case 'MOPIDY_ARTIST_LOADED':
            if( !action.data ) return Object.assign({}, ui, { artist: false })
            return Object.assign({}, ui, { artist: action.data })

        case 'LASTFM_ARTIST_LOADED':
            if( !action.data.image ) return ui

            // if we already have images, don't overwrite them
            var images = ui.artist.images
            if( images.length <= 0 ) images = action.data.image
            
            var artist = Object.assign({}, ui.artist, { images: images, bio: action.data.bio, listeners: parseInt(action.data.stats.listeners), on_tour: action.data.ontour }, )
            return Object.assign({}, ui, { artist: artist });

        case 'SPOTIFY_ARTIST_LOADED':
            if( !action.data ) return Object.assign({}, ui, { artist: false })
            return Object.assign({}, ui, { artist: action.data })

        case 'SPOTIFY_ARTIST_ALBUMS_LOADED_MORE':
            var artist = Object.assign({}, ui.artist, {
                albums: [ ...ui.artist.albums, ...action.data.items ],
                albums_more: action.data.next
            })
            return Object.assign({}, ui, { artist: artist });

        case 'ARTIST_FOLLOWING_LOADED':
            var artist = Object.assign({}, ui.artist, { following: action.is_following })
            return Object.assign({}, ui, { artist: artist, following_loading: false });


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
            var playlist = Object.assign({}, action.playlist)

            // if we already have one in our list, fetch it and update it
            if (playlists[action.playlist.uri]){
                playlist = Object.assign({}, playlists[action.playlist.uri], action.playlist)
            }

            playlists[action.playlist.uri] = playlist
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
            var tracks = Object.assign([], ui.playlist.tracks)
            var indexes = action.tracks_indexes.reverse()
            for( var i = 0; i < indexes.length; i++ ){
                tracks.splice( indexes[i], 1 )
            }
            var snapshot_id = null
            if( action.snapshot_id ) snapshot_id = action.snapshot_id
            var playlist = Object.assign({}, ui.playlist, { tracks: tracks, snapshot_id: snapshot_id })
            return Object.assign({}, ui, { playlist: playlist });

        case 'PLAYLIST_TRACKS_RESOLVED':
            var playlists = Object.assign([], ui.playlists)
            var playlist = Object.assign({}, playlists[action.uri], { tracks: action.tracks })

            playlists[action.uri] = playlist
            return Object.assign({}, ui, { playlists: playlists });

        case 'PLAYLIST_TRACKS_REORDERED':
            var snapshot_id = null
            if( action.snapshot_id ) snapshot_id = action.snapshot_id
            var tracks = Object.assign([], ui.playlist.tracks)

            // handle insert_before offset if we're moving BENEATH where we're slicing tracks
            var insert_before = action.insert_before
            if( insert_before > action.range_start ) insert_before = insert_before - action.range_length

            // cut our moved tracks into a new array
            var tracks_to_move = tracks.splice(action.range_start, action.range_length)
            tracks_to_move.reverse()

            for( i = 0; i < tracks_to_move.length; i++ ){
                tracks.splice(insert_before, 0, tracks_to_move[i])
            }

            var playlist = Object.assign({}, ui.playlist, { snapshot_id: snapshot_id, tracks: tracks })
            return Object.assign({}, ui, { playlist: playlist });

        case 'PLAYLIST_FOLLOWING_LOADED':
            var playlists = Object.assign([], ui.playlists)
            var playlist = Object.assign({}, playlists[action.uri], { following: action.is_following })

            playlists[action.uri] = playlist
            return Object.assign({}, ui, { playlists: playlists });

        case 'LIBRARY_PLAYLISTS_LOADED':
            var library_playlists = []
            if (ui.library_playlists) library_playlists = ui.library_playlists

            return Object.assign({}, ui, { 
                library_playlists: [...library_playlists, ...action.uris]
            });


        /**
         * Search results
         **/ 

        case 'SEARCH_STARTED':
            return Object.assign({}, ui, { search_results: action.data })

        case 'MOPIDY_SEARCH':

            // collate all our different sources into one array
            var tracks = []
            for( var i = 0; i < action.data.length; i++ ){
                if( action.data[i].tracks ) tracks = [...tracks, ...action.data[i].tracks]
            }

            // merge our results with all our other tracks
            var results = Object.assign({}, ui.search_results, { 
                tracks: [...ui.search_results.tracks, ...tracks]
            })
            return Object.assign({}, ui, { search_results: results })

        case 'SPOTIFY_SEARCH_RESULTS_LOADED':
            if( !action.data ) return ui

            return Object.assign({}, ui, { search_results: {
                artists: [ ...ui.search_results.artists, ...action.data.artists.items ],
                albums: [ ...ui.search_results.albums, ...action.data.albums.items ],
                playlists: [ ...ui.search_results.playlists, ...action.data.playlists.items ],
                tracks: [ ...ui.search_results.tracks, ...action.data.tracks.items ],
                artists_more: action.data.artists.next,
                albums_more: action.data.albums.next,
                playlists_more: action.data.playlists.next,
                tracks_more: action.data.tracks.next
            }});

        case 'SPOTIFY_SEARCH_RESULTS_LOADED_MORE_ARTISTS':
            var artists = [...ui.search_results.artists, ...action.data.artists.items]
            var results = Object.assign({}, ui.search_results, { 
                artists: artists,
                artists_more: action.data.artists.next
            })
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




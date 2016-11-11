
export default function reducer(ui = {}, action){
    switch (action.type) {

        case 'LAZY_LOADING':
            return Object.assign({}, ui, { lazy_loading: action.start });

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
                    context: action.context, 
                    victims: action.victims,
                    position_x: action.position_x,
                    position_y: action.position_y
                }
            });

        case 'DRAG_MOVE':
            var dragger = Object.assign({}, ui.dragger, {              
                    position_x: action.position_x,
                    position_y: action.position_y
                })
            return Object.assign({}, ui, { dragger: dragger });

        case 'DRAG_END':
            return Object.assign({}, ui, { 
                dragger: { 
                    dragging: false,
                    context: false, 
                    victims: false
                }
            });

        case 'DRAG_CANCEL':
            return Object.assign({}, ui, { 
                dragger: { 
                    dragging: false,
                    context: false, 
                    victims: false
                }
            });



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



        /**
         * Artists
         **/

        case 'MOPIDY_ARTIST_LOADED':
            if( !action.data ) return Object.assign({}, ui, { artist: false })
            return Object.assign({}, ui, { artist: action.data })

        case 'LASTFM_ARTIST_LOADED':
            if( !action.data.image ) return ui

            var artist = Object.assign({}, ui.artist, { images: action.data.image, bio: action.data.bio })
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


        /**
         * Playlists
         **/

        case 'MOPIDY_PLAYLIST_LOADED':
            if( !action.data ) return Object.assign({}, ui, { playlist: false })
            return Object.assign({}, ui, { playlist: action.data })

        case 'SPOTIFY_PLAYLIST_LOADED':
            if( !action.data ) return Object.assign({}, ui, { playlist: false })

            var tracks = []
            for( var i = 0; i < action.data.tracks.items.length; i++ ){
                tracks.push( Object.assign(
                    {},
                    action.data.tracks.items[i].track,
                    {
                        added_by: action.data.tracks.items[i].added_by,
                        added_at: action.data.tracks.items[i].added_at
                    }
                ))
            }

            var playlist = Object.assign({}, action.data, {
                tracks: tracks,
                tracks_more: action.data.tracks.next,
                tracks_total: action.data.tracks.total
            })
            return Object.assign({}, ui, { playlist: playlist });

        case 'SPOTIFY_PLAYLIST_LOADED_MORE':
            var tracks = []
            for( var i = 0; i < action.data.items.length; i++ ){
                tracks.push( Object.assign(
                    {},
                    action.data.items[i].track,
                    {
                        added_by: action.data.items[i].added_by,
                        added_at: action.data.items[i].added_at
                    }
                ))
            }

            var playlist = Object.assign({}, ui.playlist, {
                tracks: [...ui.playlist.tracks, ...tracks],
                tracks_more: action.data.next
            })
            return Object.assign({}, ui, { playlist: playlist });

        case 'SPOTIFY_PLAYLIST_FOLLOWING':
            var playlist = Object.assign({}, ui.playlist, { following: action.data })
            return Object.assign({}, ui, { playlist: playlist });

        case 'SPOTIFY_PLAYLIST_TRACKS_REMOVED':
            var tracks = Object.assign([], ui.playlist.tracks)
            for( var i = 0; i < action.positions.length; i++ ){
                tracks.splice( action.positions[i], 1 )
            }
            var playlist = Object.assign({}, ui.playlist, action.snapshot_id, { tracks: tracks })
            return Object.assign({}, ui, { playlist: playlist });


        /**
         * Library Playlists
         *
         * TODO: Map sources and merge any replicated URIS
         *
         **/

        case 'MOPIDY_PLAYLISTS_LOADED':
            if( !action.data ) return ui
            return Object.assign({}, ui, { 
                playlists: [...ui.playlists, ...action.data],
            });

        case 'SPOTIFY_LIBRARY_PLAYLISTS_LOADED':
            if( !action.data ) return ui
            return Object.assign({}, ui, { 
                playlists: [...ui.playlists, ...action.data]
            });



        /**
         * Current track and tracklist
         **/

        case 'MOPIDY_TLTRACKS':
            if( !action.data ) return ui

            var tracklist = []
            for( var i = 0; i < action.data.length; i++ ){
                tracklist.push( Object.assign({}, action.data[i].track, { tlid: action.data[i].tlid }) )
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


        default:
            return ui
    }
}




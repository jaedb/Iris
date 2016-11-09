
export default function reducer(ui = {}, action){
    switch (action.type) {

        case 'UI_SHOW_CONTEXT_MENU':
            return Object.assign({}, ui, { 
            	context_menu: { 
            		show: true,
            		position_x: action.position_x,
            		position_y: action.position_y,
            		context: action.context, 
            		data: action.data
            	}
            });

        case 'UI_HIDE_CONTEXT_MENU':
            return Object.assign({}, ui, { context_menu: { show: false } });

        case 'UI_LAZY_LOADING':
            return Object.assign({}, ui, { lazy_loading: action.start });

        case 'SPOTIFY_ALBUM_LOADED':
        case 'MOPIDY_ALBUM_LOADED':
            return Object.assign({}, ui, { album: action.data });

        case 'LASTFM_ALBUM_LOADED':
            //if( !action.data.image ) return ui
            var album = Object.assign(ui.album, { images: action.data.image })
            return Object.assign({}, ui, { album: album });

        case 'SPOTIFY_ALBUM_LOADED_MORE':
            var album = ui.album
            Object.assign(album, { tracks: {
                href: action.data.href,
                next: action.data.next,
                previous: action.data.previous,
                items: [ ...ui.album.tracks.items, ...action.data.items ]
            }})
            return Object.assign({}, ui, { album: album });


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

        default:
            return ui
    }
}




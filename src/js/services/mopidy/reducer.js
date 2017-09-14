
import * as helpers from '../../helpers'

export default function reducer(mopidy = {}, action){
    switch (action.type) {

        case 'MOPIDY_CONNECT':
        case 'MOPIDY_CONNECTING':
            return Object.assign({}, mopidy, { connected: false, connecting: true });

        case 'MOPIDY_CONNECTED':
            return Object.assign({}, mopidy, { connected: true, connecting: false });

        case 'MOPIDY_DISCONNECTED':
            return Object.assign({}, mopidy, { connected: false, connecting: false });

        case 'MOPIDY_SET_CONFIG':
            return Object.assign({}, mopidy, {
                host: action.host, 
                port: action.port,
                ssl: action.ssl
            });

        case 'MOPIDY_CHANGE_TRACK':
            return Object.assign({}, mopidy, {
                tlid: action.tlid
            });

        case 'MOPIDY_URISCHEMES_FILTERED':
            return Object.assign({}, mopidy, {
                uri_schemes: action.data
            });


        /**
         * State-oriented actions
         **/
        case 'MOPIDY_STATE':
            return Object.assign({}, mopidy, {
                play_state: action.data 
            });

        case 'MOPIDY_CONSUME':
            return Object.assign({}, mopidy, {
                consume: action.data 
            });

        case 'MOPIDY_RANDOM':
            return Object.assign({}, mopidy, {
                random: action.data 
            });

        case 'MOPIDY_REPEAT':
            return Object.assign({}, mopidy, {
                repeat: action.data 
            });

        case 'MOPIDY_VOLUME':
            return Object.assign({}, mopidy, {
                volume: action.data   
            });

        case 'MOPIDY_MUTE':
            return Object.assign({}, mopidy, {
                mute: action.data
            });

        case 'MOPIDY_TIMEPOSITION':
            return Object.assign({}, mopidy, {
                time_position: action.data
            });

        case 'MOPIDY_HISTORY':
            var history = []
            for (var i = 0; i < action.data.length; i++){
                history.push(Object.assign(
                    {},
                    action.data[i][1],
                    {
                        played_at: action.data[i][0],
                        type: 'history'
                    }
                ))
            }
            return Object.assign({}, mopidy, {
                queue_history: history
            });


        /**
         * Asset-oriented actions
         **/

        case 'MOPIDY_DIRECTORY_LOADED':
            return Object.assign({}, mopidy, {
                directory: action.data   
            });


        /**
         * Library
         **/

        case 'MOPIDY_LIBRARY_PLAYLISTS_LOADED':
            if (mopidy.library_playlists){
                var uris = [...mopidy.library_playlists,...action.uris]
            } else {
                var uris = action.uris
            }
            return Object.assign({}, mopidy, { library_playlists: helpers.removeDuplicates(uris) })

        case 'MOPIDY_LIBRARY_PLAYLISTS_LOADED_ALL':
            return Object.assign({}, mopidy, { library_playlists_loaded_all: true })

        case 'MOPIDY_LIBRARY_PLAYLIST_CREATED':
            var library_playlists = []
            if (mopidy.library_playlists){
                library_playlists = Object.assign([], mopidy.library_playlists)
                library_playlists.push(action.key)
            }
            return Object.assign({}, mopidy, { library_playlists: library_playlists })

        case 'MOPIDY_LIBRARY_PLAYLIST_DELETED':
            var library_playlists = []
            if (mopidy.library_playlists){
                library_playlists = Object.assign([], mopidy.library_playlists)
                library_playlists.splice(library_playlists.indexOf(action.uri), 1)
            }
            return Object.assign({}, mopidy, { library_playlists: library_playlists })

        case 'MOPIDY_LIBRARY_ARTISTS_LOADED':
            if (mopidy.library_artists){
                var uris = [...mopidy.library_artists,...action.uris]
            } else {
                var uris = action.uris
            }
            return Object.assign({}, mopidy, { library_artists: helpers.removeDuplicates(uris) })

        case 'MOPIDY_LIBRARY_ALBUMS_LOADED':
            if (mopidy.library_albums){
                var uris = [...mopidy.library_albums,...action.uris]
            } else {
                var uris = action.uris
            }
            return Object.assign({}, mopidy, { library_albums: helpers.removeDuplicates(uris) })


        /**
         * Searching
         **/

        case 'MOPIDY_CLEAR_SEARCH_RESULTS':
            return Object.assign({}, mopidy, { search_results: {} });

        case 'MOPIDY_SEARCH_RESULTS_LOADED':

            // Fetch or create our container
            if (mopidy.search_results){
                var search_results = Object.assign({}, mopidy.search_results)
            } else {
                var search_results = {}
            }

            if (search_results[action.context]){
                search_results[action.context] = [...search_results[action.context], ...action.results]
            } else {
                search_results[action.context] = action.results
            }

            return Object.assign({}, mopidy, { search_results: search_results });

        default:
            return mopidy
    }
}




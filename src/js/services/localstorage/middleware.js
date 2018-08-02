
var helpers = require('../../helpers.js')

const localstorageMiddleware = (function(){

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {

        // proceed as normal first
        // this way, any reducers and middleware do their thing BEFORE we store our new state
        next(action);

        // append our state to a global variable. This gives us access to debug the store at any point
        window._store = store

        // if debug enabled
        if (store.getState().ui.log_actions){

            var ignored_actions = [
                'START_LOADING',
                'STOP_LOADING'
            ]

            // Show non-ignored actions
            if (!ignored_actions.includes(action.type)){
                console.log(action)
            }
        }

        switch(action.type){

            case 'PUSHER_CONNECTED':
                helpers.setStorage(
                    'pusher', 
                    {
                        connection_id: action.connection_id
                    }
                );
                break;

            case 'PUSHER_SET_PORT':
                helpers.setStorage(
                    'pusher', 
                    {
                        port: action.port
                    }
                );
                break;

            case 'PUSHER_USERNAME_CHANGED':
                helpers.setStorage(
                    'pusher', 
                    {
                        username: action.username
                    }
                );
                break;

            case 'MOPIDY_URISCHEMES_FILTERED':
                helpers.setStorage(
                    'mopidy', 
                    {
                        uri_schemes: action.data
                    }
                );
                break;

            case 'SPOTIFY_IMPORT_AUTHORIZATION':
            case 'SPOTIFY_AUTHORIZATION_GRANTED':
                if (action.authorization !== undefined){
                    var authorization = action.authorization;
                } else if (action.data){
                    var authorization = action.data;
                }
                helpers.setStorage(
                    'spotify', 
                    {
                        authorization: authorization,
                        access_token: authorization.access_token, 
                        refresh_token: authorization.refresh_token, 
                        token_expiry: authorization.token_expiry
                    }
                );
                break;

            case 'SPOTIFY_AUTHORIZATION_REVOKED':
                helpers.setStorage(
                    'spotify', 
                    {
                        authorization: false, 
                        access_token: false, 
                        refresh_token: false, 
                        token_expiry: false
                    }
                );
                break;

            case 'SPOTIFY_TOKEN_REFRESHED':
                helpers.setStorage(
                    'spotify', 
                    {
                        access_token: action.data.access_token,
                        token_expiry: action.data.token_expiry,
                        provider: action.provider
                    }
                );
                break;

            case 'SPOTIFY_ME_LOADED':
                helpers.setStorage(
                    'spotify', 
                    {
                        me: action.data
                    }
                );
                break;

            case 'CORE_SET':
                helpers.setStorage(
                    'core', 
                    action.data
                );
                break

            case 'UI_SET':
                helpers.setStorage(
                    'ui', 
                    action.data
                );
                break

            case 'MOPIDY_SET':
                helpers.setStorage(
                    'mopidy', 
                    action.data
                );
                break;

            case 'SPOTIFY_SET':
                helpers.setStorage(
                    'spotify', 
                    action.data
                );
                break;

            case 'SUPPRESS_BROADCAST':
                var ui = helpers.getStorage('ui');
                if (ui.suppressed_broadcasts !== undefined){
                    var suppressed_broadcasts = ui.suppressed_broadcasts;
                } else {
                    var suppressed_broadcasts = [];
                }

                suppressed_broadcasts.push(action.key);

                helpers.setStorage(
                    'ui', 
                    {
                        suppressed_broadcasts: suppressed_broadcasts
                    }
                );
                break

            case 'LASTFM_AUTHORIZATION_GRANTED':
                helpers.setStorage(
                    'lastfm', 
                    {
                        session: action.data.session
                    }
                );
                break;

            case 'LASTFM_AUTHORIZATION_REVOKED':
                helpers.setStorage(
                    'lastfm', 
                    {
                        session: null
                    }
                );
                break;
            
            /**
             * Experimental saving of stores to localStorage
             * This uses way too much storage space (ie 10MB+) so won't work. We need
             * to use the IndexedDB engine instead for storing this quantity of data

            case 'UPDATE_TRACKS_INDEX':
                helpers.setStorage('core', {tracks: action.tracks});
                next(action);
                break;
            case 'UPDATE_ALBUMS_INDEX':
                helpers.setStorage('core', {albums: action.albums});
                next(action);
                break;
            case 'UPDATE_ARTISTS_INDEX':
                helpers.setStorage('core', {artists: action.artists});
                next(action);
                break;
            case 'UPDATE_PLAYLISTS_INDEX':
                helpers.setStorage('core', {playlists: action.playlists});
                next(action);
                break;
            case 'UPDATE_USERS_INDEX':
                helpers.setStorage('core', {users: action.users});
                next(action);
                break;
             */
        }
    }

})();

export default localstorageMiddleware
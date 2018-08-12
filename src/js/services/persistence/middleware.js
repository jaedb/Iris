
import Dexie from 'dexie';

var helpers = require('../../helpers.js');
var coreActions = require('../core/actions.js');

const persistenceMiddleware = (function(){

    var db = new Dexie("iris");
    db.version(1).stores({
        tracks: "&uri,name",
        albums: "&uri,name",
        artists: "&uri,name",
        playlists: "&uri,name",
        users: "&uri"
    });

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {

        // proceed as normal first
        // this way, any reducers and middleware do their thing BEFORE we store our new state
        //next(action);

        // append our state to a global variable. This gives us access to debug the store at any point
        window._store = store;

        // Add the ability to echo out our database
        window._db = db;

        // Add the ability to echo out our database
        window._db_table = function(table){
            db[table].toArray()
                .then(
                    result => {
                        console.log(result);
                    }
                );
        }

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
            
            case 'LOAD_ALBUM':
                db.albums.get(action.uri)
                    .then(
                        album => {
                            if (album && album.complete){
                                console.log('Restoring album from persistent store');
                                store.dispatch(coreActions.albumLoaded(album));

                                // We need to load all it's tracks too
                                if (album.tracks_uris){
                                    album.tracks_uris.forEach(track_uri => {
                                        store.dispatch(coreActions.loadTrack(track_uri));
                                    });
                                }
                            } else {
                                next(action);
                            }
                        }
                    );
                break;
            
            case 'LOAD_ARTIST':
                db.artists.get(action.uri)
                    .then(
                        artist => {
                            if (artist && artist.complete){
                                console.log('Restoring artist from persistent store', artist);
                                store.dispatch(coreActions.artistLoaded(artist));

                                // We need to load all it's tracks too
                                if (artist.tracks_uris){
                                    artist.tracks_uris.forEach(track_uri => {
                                        store.dispatch(coreActions.loadTrack(track_uri));
                                    });
                                }

                                // We need to load all it's albums too
                                if (artist.albums_uris){
                                    artist.albums_uris.forEach(album_uri => {
                                        store.dispatch(coreActions.loadAlbum(album_uri));
                                    });
                                }
                            } else {
                                next(action);
                            }
                        }
                    );
                break;
            
            case 'LOAD_PLAYLIST':
                db.playlists.get(action.uri)
                    .then(
                        playlist => {
                            if (playlist && playlist.complete){
                                console.log('Restoring playlist from persistent store');
                                store.dispatch(coreActions.playlistLoaded(playlist));
                            } else {
                                next(action);
                            }
                        }
                    );
                break;
            
            case 'LOAD_TRACK':
                db.tracks.get(action.uri)
                    .then(
                        track => {
                            if (track){
                                console.log('Restoring track from persistent store');
                                store.dispatch(coreActions.trackLoaded(track));
                            } else {
                                next(action);
                            }
                        }
                    );
                break;
            
            case 'TRACKS_LOADED':
                db.transaction('rw', db.tracks, () => {
                    db.tracks.bulkPut(action.tracks);
                }).catch(function (e) {
                    store.dispatch(coreActions.handleException("Failed to update tracks table", e));
                });

                next(action);
                break;
            
            case 'ARTISTS_LOADED':
                db.transaction('rw', db.artists, () => {
                    db.artists.bulkPut(action.artists);
                }).catch(function (e) {
                    store.dispatch(coreActions.handleException("Failed to update artists table", e));
                });

                next(action);
                break;
            
            case 'ALBUMS_LOADED':
                db.transaction('rw', db.albums, () => {
                    db.albums.bulkPut(action.albums);
                }).catch(function (e) {
                    store.dispatch(coreActions.handleException("Failed to update albums table", e));
                });

                next(action);
                break;
            
            case 'PLAYLISTS_LOADED':
                db.transaction('rw', db.playlists, () => {
                    db.playlists.bulkPut(action.playlists);
                }).catch(function (e) {
                    store.dispatch(coreActions.handleException("Failed to update playlists table", e));
                });

                next(action);
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

            default:
                next(action);
                break;
        }
    }

})();

export default persistenceMiddleware
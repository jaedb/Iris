
import ReactGA from 'react-ga'

var coreActions = require('./actions.js');
var uiActions = require('../ui/actions.js');
var pusherActions = require('../pusher/actions.js');
var mopidyActions = require('../mopidy/actions.js');
var spotifyActions = require('../spotify/actions.js');
var lastfmActions = require('../lastfm/actions.js');
var helpers = require('../../helpers.js');

const CoreMiddleware = (function(){

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {
        var core = store.getState().core;

        switch(action.type){

            case 'HANDLE_EXCEPTION':

                // Construct meaningful message and description
                var message = action.message;
                if (action.description){
                    var description = action.description;
                } else if (action.data.xhr && action.data.xhr.responseText){
                    var xhr_response = JSON.parse(action.data.xhr.responseText);        
                    if (xhr_response.error && xhr_response.error.message){
                        var description = xhr_response.error.message;
                    }
                } else {
                    var description = null;
                }

                // Prepare a summary dump of our state
                var state = store.getState();
                var exported_state = {
                    core: Object.assign({},state.core),
                    ui: Object.assign({},state.ui),
                    spotify: Object.assign({},state.spotify),
                    mopidy: Object.assign({},state.mopidy),
                    pusher: Object.assign({},state.pusher)
                }

                // Strip out non-essential store info
                delete exported_state.core.albums
                delete exported_state.core.artists
                delete exported_state.core.playlists
                delete exported_state.core.users
                delete exported_state.core.queue_metadata
                delete exported_state.core.current_tracklist
                delete exported_state.spotify.library_albums
                delete exported_state.spotify.library_artists
                delete exported_state.spotify.library_playlists
                delete exported_state.spotify.autocomplete_results
                delete exported_state.mopidy.library_albums
                delete exported_state.mopidy.library_artists
                delete exported_state.mopidy.library_playlists

                var data = Object.assign(
                    {},
                    action.data, 
                    {
                        message: message,
                        description: description,
                        state: exported_state
                    }
                );

                // Log with Raven Sentry
                Raven.captureException(
                    new Error(message), 
                    {
                        extra: data
                    }
                );

                // Log with Analytics
                ReactGA.event({
                    category: "Error",
                    action: message,
                    label: description,
                    nonInteraction: true
                });

                store.dispatch(uiActions.createNotification(
                    message, 
                    'bad',
                    null, 
                    null, 
                    description
                ));
                console.error(message, description, data);
                break;

            case 'CORE_START_SERVICES':
                store.dispatch(mopidyActions.connect());
                store.dispatch(pusherActions.connect());

                next(action)
                break

            case 'PLAY_PLAYLIST':
                ReactGA.event({ category: 'Playlist', action: 'Play', label: action.uri })
                next(action)
                break

            case 'SAVE_PLAYLIST':
                ReactGA.event({ category: 'Playlist', action: 'Save', label: action.key })
                next(action)
                break

            case 'CREATE_PLAYLIST':
                ReactGA.event({ category: 'Playlist', action: 'Create', label: +action.name })
                next(action)
                break

            case 'REORDER_PLAYLIST_TRACKS':
                ReactGA.event({ category: 'Playlist', action: 'Reorder tracks', label: action.key })
                next(action)
                break

            case 'ADD_PLAYLIST_TRACKS':
                ReactGA.event({ category: 'Playlist', action: 'Add tracks', label: action.playlist_uri })
                next(action)
                break

            case 'REMOVE_PLAYLIST_TRACKS':
                ReactGA.event({ category: 'Playlist', action: 'Remove tracks', label: action.playlist_uri })
                next(action)
                break

            case 'DELETE_PLAYLIST':
                ReactGA.event({ category: 'Playlist', action: 'Delete', label: action.uri })
                next(action)
                break

            case 'SEARCH_STARTED':
                ReactGA.event({ category: 'Search', action: 'Started', label: action.type+': '+action.query })
                next(action)

                var state = store.getState()
                if (state.ui.search_uri_schemes){
                    var uri_schemes = state.ui.search_uri_schemes
                } else {
                    var uri_schemes = state.mopidy.uri_schemes
                }

                // backends that can handle more than just track results
                // make sure they are available and respect our settings
                var available_full_uri_schemes = ['local:','file:','gmusic:']
                var full_uri_schemes = []
                for (var i = 0; i < available_full_uri_schemes.length; i++){
                    var index = uri_schemes.indexOf(available_full_uri_schemes[i])
                    if (index > -1){
                        full_uri_schemes.push(available_full_uri_schemes[i])
                    }
                }

                // initiate spotify searching
                if (!action.only_mopidy){
                    if (!state.ui.search_settings || state.ui.search_settings.spotify){
                        store.dispatch(spotifyActions.getSearchResults(action.query))
                    }
                }

                // backend searching (mopidy)
                if (state.mopidy.connected){
                    store.dispatch(mopidyActions.getSearchResults(action.search_type, action.query, 100, full_uri_schemes))
                }

                break

            case 'PLAYLIST_TRACKS_ADDED':
                store.dispatch(uiActions.createNotification('Added '+action.tracks_uris.length+' tracks to playlist'))                
                switch(helpers.uriSource(action.key)){
                    case 'spotify':
                        store.dispatch(spotifyActions.getPlaylist(action.key))
                        break
                    case 'm3u':
                        if (store.getState().mopidy.connected ) store.dispatch(mopidyActions.getPlaylist(action.key))
                        break
                }
                next(action)
                break

            case 'MOPIDY_CURRENTTLTRACK':
                if (action.data && action.data.track){
                    helpers.setWindowTitle(action.data.track, store.getState().mopidy.play_state);

                    // make sure our images use mopidy host:port
                    if (action.data.track.album && action.data.track.album.images && action.data.track.album.images.length > 0){
                        var images = Object.assign([], action.data.track.album.images)
                        for (var i = 0; i < images.length; i++){
                            if (typeof(images[i]) === 'string' && images[i].startsWith('/images/')){
                                images[i] = '//'+store.getState().mopidy.host+':'+store.getState().mopidy.port+images[i]
                            }
                        }
                        action.data.track.album.images = images
                    }
                }

                var current_tracklist = []
                Object.assign(current_tracklist, core.current_tracklist)

                for (var i = 0; i < current_tracklist.length; i++){
                    Object.assign(
                        current_tracklist[i], 
                        { playing: (current_tracklist[i].tlid == action.data.tlid) }
                    )
                }

                // Update action
                action.current_tracklist = current_tracklist;
                action.current_track = action.data.track.uri;

                next(action);
                break;

            case 'MOPIDY_TLTRACKS':
                var core = store.getState().core;
                var tracklist = []
                for (var i = 0; i < action.data.length; i++){

                    var tltrack = helpers.formatTracks(action.data[i]);

                    // load our metadata (if we have any for that tlid)
                    if (core.queue_metadata !== undefined && core.queue_metadata['tlid_'+tltrack.tlid] !== undefined){
                        var metadata = core.queue_metadata['tlid_'+tltrack.tlid]
                    } else {
                        var metadata = {}
                    }

                    var current_tlid = null;
                    if (core.current_track && core.tracks && core.tracks[core.current_track] !== undefined && core.tracks[core.current_track].tlid !== undefined){
                        current_tlid = core.tracks[core.current_track].tlid;
                    }

                    var track = Object.assign(
                        {}, 
                        tltrack,
                        metadata,
                        {
                            playing: (tltrack.tlid == current_tlid)
                        })
                    tracklist.push(track)
                }

                // Append to our action
                tracklist = helpers.formatTracks(tracklist);
                action.tracklist = tracklist;

                // Load our tracks into index
                store.dispatch({
                    type: 'TRACKS_LOADED',
                    tracks: tracklist
                });

                next(action);
                break

            // Get assets from all of our providers
            case 'GET_LIBRARY_PLAYLISTS':
                if (store.getState().spotify.connected){
                    store.dispatch(spotifyActions.getLibraryPlaylists())
                }
                if (store.getState().mopidy.connected){
                    store.dispatch(mopidyActions.getLibraryPlaylists())
                }
                next(action)
                break

            // Get assets from all of our providers
            case 'GET_LIBRARY_ALBUMS':
                if (store.getState().spotify.connected){
                    store.dispatch(spotifyActions.getLibraryAlbums())
                }
                if (store.getState().mopidy.connected){
                    store.dispatch(mopidyActions.getLibraryAlbums())
                }
                next(action)
                break

            // Get assets from all of our providers
            case 'GET_LIBRARY_ARTISTS':
                if (store.getState().spotify.connected){
                    store.dispatch(spotifyActions.getLibraryArtists())
                }
                if (store.getState().mopidy.connected){
                    store.dispatch(mopidyActions.getLibraryArtists())
                }
                next(action)
                break

            case 'RESTART':
                location.reload()
                break


            /**
             * Index actions
             * These modify our asset indexes, which are used globally
             **/

            // Array wrapper for TRACKS_LOADED
            case 'TRACK_LOADED':
                store.dispatch({
                    type: 'TRACKS_LOADED',
                    tracks: [action.track]
                })
                next(action);
                break;

            // Array wrapper for ALBUMS_LOADED
            case 'ALBUM_LOADED':
                store.dispatch({
                    type: 'ALBUMS_LOADED',
                    albums: [action.album]
                });
                next(action);
                break;

            // Array wrapper for ARTISTS_LOADED
            case 'ARTIST_LOADED':
                store.dispatch({
                    type: 'ARTISTS_LOADED',
                    artists: [action.artist]
                });
                next(action);
                break;

            // Array wrapper for PLAYLISTS_LOADED
            case 'PLAYLIST_LOADED':
                store.dispatch({
                    type: 'PLAYLISTS_LOADED',
                    playlists: [action.playlist]
                });
                next(action);
                break;

            // Array wrapper for USERS_LOADED
            case 'USER_LOADED':
                store.dispatch({
                    type: 'USERS_LOADED',
                    users: [action.user]
                });
                next(action);
                break;

            case 'TRACKS_LOADED':
                var tracks = Object.assign({}, core.tracks);

                for (var i = 0; i < action.tracks.length; i++){
                    var track = Object.assign({}, helpers.formatTracks(action.tracks[i]));

                    if (tracks[track.uri]){
                        track = Object.assign({}, tracks[track.uri], track);
                    }

                    if (track.album && track.album.images && track.album.images.length > 0){
                        track.album.images = helpers.digestMopidyImages(store.getState().mopidy, track.album.images);
                    }

                    tracks[track.uri] = track;
                }

                // Update index
                store.dispatch({
                    type: 'UPDATE_TRACKS_INDEX',
                    tracks: tracks
                });

                next(action);
                break;

            case 'ALBUMS_LOADED':
                var albums = Object.assign({}, core.albums);
                var tracks_loaded = [];

                for (var i = 0; i < action.albums.length; i++){
                    var album = Object.assign({}, action.albums[i]);

                    if (albums[album.uri]){
                        album = Object.assign({}, albums[album.uri], album);
                    }

                    if (album.images && album.images.length > 0){
                        album.images = helpers.digestMopidyImages(store.getState().mopidy, album.images);
                    }

                    // Load our tracks
                    if (album.tracks){
                        var tracks = helpers.formatTracks(album.tracks);
                        var tracks_uris = helpers.arrayOf('uri', tracks);
                        album.tracks_uris = tracks_uris;
                        delete album.tracks;
                        tracks_loaded = [...tracks_loaded, ...tracks];
                    }

                    albums[album.uri] = album;
                }

                // Load these new tracks
                store.dispatch({
                    type: 'TRACKS_LOADED',
                    tracks: tracks_loaded
                });

                // Update index
                store.dispatch({
                    type: 'UPDATE_ALBUMS_INDEX',
                    albums: albums
                });

                next(action);
                break

            case 'ARTISTS_LOADED':
                var artists = Object.assign({}, core.artists);
                var tracks_loaded = [];

                for (var i = 0; i < action.artists.length; i++){
                    var artist = action.artists[i];

                    if (artists[artist.uri]){

                        // if we've already got images, remove and add as additional_images
                        // this is to prevent LastFM overwriting Spotify images
                        if (artists[artist.uri].images){
                            artist.images_additional = artist.images
                            delete artist.images
                        }

                        artist = Object.assign({}, artists[artist.uri], artist);
                    }

                    if (artist.tracks){
                        var tracks = helpers.formatTracks(artist.tracks);
                        var tracks_uris = helpers.arrayOf('uri', tracks);
                        artist.tracks_uris = tracks_uris;
                        delete artist.tracks;
                        tracks_loaded = [...tracks_loaded, ...tracks];
                    }

                    // Update index
                    artists[artist.uri] = artist;
                }

                // Load our tracks
                store.dispatch({
                    type: 'TRACKS_LOADED',
                    tracks: tracks_loaded
                });

                store.dispatch({
                    type: 'UPDATE_ARTISTS_INDEX',
                    artists: artists
                });

                next(action);
                break;

            case 'PLAYLISTS_LOADED':
                var playlists = Object.assign({}, core.playlists);
                var tracks_loaded = [];

                for (var i = 0; i < action.playlists.length; i++){
                    var playlist = Object.assign({}, action.playlists[i]);

                    // Detect editability
                    switch (helpers.uriSource(playlist.uri)){

                        case 'm3u':
                            playlist.can_edit = true
                            break

                        case 'spotify':
                            if (store.getState().spotify.authorization && store.getState().spotify.me){
                                playlist.can_edit = (helpers.getFromUri('playlistowner',playlist.uri) == store.getState().spotify.me.id)
                            }
                    }

                    if (playlists[playlist.uri] !== undefined){
                        playlist = Object.assign({}, playlists[playlist.uri], playlist);
                    }

                    // Load our tracks
                    if (playlist.tracks){
                        var tracks = helpers.formatTracks(playlist.tracks);
                        var tracks_uris = helpers.arrayOf('uri', tracks);
                        playlist.tracks_uris = tracks_uris;
                        delete playlist.tracks;
                        tracks_loaded = [...tracks_loaded, ...tracks];
                    }

                    // Update index
                    playlists[playlist.uri] = playlist;
                }

                // Load our tracks
                store.dispatch({
                    type: 'TRACKS_LOADED',
                    tracks: tracks_loaded
                });

                store.dispatch({
                    type: 'UPDATE_PLAYLISTS_INDEX',
                    playlists: playlists
                });

                next(action);
                break;

            case 'USERS_LOADED':
                var users = Object.assign({}, core.users);

                for (var i = 0; i < action.users.length; i++){
                    var user = Object.assign({}, action.users[i]);

                    if (users[user.uri]){
                        user = Object.assign({}, users[user.uri], user);
                    }

                    users[action.key] = user;
                }

                // Update index
                store.dispatch({
                    type: 'UPDATE_USERS_INDEX',
                    users: users
                });

                next(action);
                break;

            /**
             * Loaded more linked assets
             * Often fired during lazy-loading or async asset grabbing.
             * We link the parent to these indexed records by {type}s_uris
             **/

            case 'LOADED_MORE':
                var parent_type_plural = action.parent_type+'s';
                var parent_index = Object.assign({}, core[action.parent_type+'s']);
                var parent = Object.assign({}, parent_index[action.parent_key]);

                if (action.records_data.items !== undefined){
                    var records = action.records_data.items;
                } else if (action.records_data.tracks !== undefined){
                    var records = action.records_data.tracks;
                } else if (action.records_data.artists !== undefined){
                    var records = action.records_data.artists;
                } else if (action.records_data.albums !== undefined){
                    var records = action.records_data.albums;
                } else if (action.records_data.playlists !== undefined){
                    var records = action.records_data.playlists;
                } else {
                    var records = action.records_data;
                }

                if (action.records_type == 'track'){
                    records = helpers.formatTracks(records);
                }

                var records_type_plural = action.records_type+'s';
                var records_index = Object.assign({});
                var records_uris = helpers.arrayOf('uri', records);

                // Append our records_uris array with our new records
                var uris = [...parent[records_type_plural+'_uris'], ...records_uris];
                parent[records_type_plural+'_uris'] = uris;
                if (action.records_data.next !== undefined){
                    parent[records_type_plural+'_more'] = action.records_data.next;
                }

                // Parent loaded (well, changed)
                var parent_action = {
                    type: parent_type_plural.toUpperCase()+'_LOADED'
                };
                parent_action[parent_type_plural] = [parent];
                store.dispatch(parent_action);

                // Records loaded
                var records_action = {
                    type: records_type_plural.toUpperCase()+'_LOADED'
                };
                records_action[records_type_plural] = records;
                store.dispatch(records_action);

                next(action);
                break;

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action)
        }
    }

})();

export default CoreMiddleware
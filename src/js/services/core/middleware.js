
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
                store.dispatch(lastfmActions.connect());

                next(action)
                break

            case 'TRACK_LOADED':
                if (action.data) ReactGA.event({ category: 'Track', action: 'Load', label: action.key });

                if (action.track.album && action.track.album.images && action.track.album.images.length > 0){
                    action.track.album.images = helpers.digestMopidyImages(store.getState().mopidy, action.track.album.images);
                }

                next(action)
                break

            case 'ALBUM_LOADED':
                if (action.data) ReactGA.event({ category: 'Album', action: 'Load', label: action.key })

                if (action.album.images && action.album.images.length > 0){
                    action.album.images = helpers.digestMopidyImages(store.getState().mopidy, action.album.images);
                }

                next(action)
                break

            case 'ALBUMS_LOADED':
                if (action.data) ReactGA.event({ category: 'Albums', action: 'Load', label: action.albums.length+' items' })

                for (var i = 0; i < action.albums.length; i++){
                    if (action.albums[i].images && action.albums[i].images.length > 0){
                        action.albums[i].images = helpers.digestMopidyImages(store.getState().mopidy, action.albums[i].images);
                    }
                }

                next(action)
                break

            case 'ARTIST_LOADED':
                if (action.data) ReactGA.event({ category: 'Artist', action: 'Load', label: action.artist.uri })
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

            case 'PLAYLIST_LOADED':
                if (action.data) ReactGA.event({ category: 'Playlist', action: 'Load', label: action.playlist.uri })

                var playlist = Object.assign({}, action.playlist)
                switch (helpers.uriSource(playlist.uri)){

                    case 'm3u':
                        playlist.can_edit = true
                        break

                    case 'spotify':
                        if (store.getState().spotify.authorization && store.getState().spotify.me){
                            playlist.can_edit = (helpers.getFromUri('playlistowner',playlist.uri) == store.getState().spotify.me.id)
                        }
                }

                // proceed as usual
                action.playlist = playlist
                next(action)
                break

            case 'PLAYLISTS_LOADED':
                if (action.data) ReactGA.event({ category: 'Playlists', action: 'Load', label: action.playlists.length+' items' })

                var playlists = []
                for (var i = 0; i < action.playlists.length; i++){
                    var playlist = Object.assign({}, action.playlists[i])

                    switch (helpers.uriSource(playlist.uri)){

                        case 'm3u':
                            playlist.can_edit = true
                            break

                        case 'spotify':
                            if (store.getState().spotify.authorization && store.getState().spotify.me){
                                playlist.can_edit = (helpers.getFromUri('playlistowner',playlist.uri) == store.getState().spotify.me.id)
                            }
                    }

                    playlists.push(playlist)
                }

                // proceed as usual
                action.playlists = playlists
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

                next(action)
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

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action)
        }
    }

})();

export default CoreMiddleware
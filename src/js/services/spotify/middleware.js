
var helpers = require('./../../helpers.js')
var spotifyActions = require('./actions.js')
var uiActions = require('../ui/actions.js')

const SpotifyMiddleware = (function(){

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {
        var state = store.getState();

        switch(action.type){

            case 'SPOTIFY_CONNECT':
                store.dispatch( spotifyActions.getMe() )
                break

            case 'SPOTIFY_CREATE_PLAYLIST':
                var playlist = state.ui.playlist

                if( !store.getState().spotify.authorized ){
                    store.dispatch( uiActions.createNotification( "Must be logged in to Spotify to do that", 'bad' ) )
                    return
                }
                store.dispatch( spotifyActions.createPlaylist( action.name, action.is_private ))
                break

            case 'SPOTIFY_REMOVE_PLAYLIST_TRACKS':
                var playlist = state.ui.playlist

                if( !store.getState().spotify.authorized ){
                    store.dispatch( uiActions.createNotification( "Must be logged in to Spotify to do that", 'bad' ) )
                    return
                }
                if( !store.getState().spotify.me || store.getState().spotify.me.id != playlist.owner.id ){
                    store.dispatch( uiActions.createNotification( "You can't edit a playlist you don't own", 'bad' ) )
                    return
                }
                store.dispatch( spotifyActions.deleteTracksFromPlaylist( playlist.uri, playlist.snapshot_id, action.tracks_indexes ))
                break


            case 'SPOTIFY_ADD_PLAYLIST_TRACKS':

                if( !store.getState().spotify.authorized ){
                    store.dispatch( uiActions.createNotification( "Must be logged in to Spotify to do that", 'bad' ) )
                    return
                }
                store.dispatch( spotifyActions.addTracksToPlaylist( action.playlist_uri, action.tracks_uris ))
                break


            case 'SPOTIFY_REORDER_PLAYLIST_TRACKS':

                if( !store.getState().spotify.authorized ){
                    store.dispatch( uiActions.createNotification( "Must be logged in to Spotify to do that", 'bad' ) )
                    return
                }
                store.dispatch( spotifyActions.reorderPlaylistTracks( action.uri, action.range_start, action.range_length, action.insert_before, action.snapshot_id ))
                break


            case 'SPOTIFY_SAVE_PLAYLIST':

                if( !store.getState().spotify.authorized ){
                    store.dispatch( uiActions.createNotification( "Must be logged in to Spotify to do that", 'bad' ) )
                    return
                }
                store.dispatch( spotifyActions.savePlaylist( action.uri, action.name, action.is_public ))
                break

            // when our mopidy server current track changes
            case 'MOPIDY_CURRENTTLTRACK':

                // proceed as usual so we don't inhibit default functionality
                next(action)

                // if the current track is a spotify track
                if( action.data && action.data.track.uri.substring(0,14) == 'spotify:track:' ){
                    store.dispatch( spotifyActions.getTrack( action.data.track.uri ) )
                }
                break

            // when our mopidy server current track changes
            case 'PUSHER_RADIO':

                // proceed as usual so we don't inhibit default functionality
                next(action)

                // only resolve if radio is enabled
                if( action.data.radio.enabled ){
                    store.dispatch( spotifyActions.resolveRadioSeeds( action.data.radio ) )
                }
                break

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default SpotifyMiddleware
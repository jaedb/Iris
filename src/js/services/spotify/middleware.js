

var actions = require('./actions.js')

const SpotifyMiddleware = (function(){

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {
        var state = store.getState();

        switch(action.type){

            case 'SPOTIFY_CONNECT':
                store.dispatch( actions.getMe() )


            case 'SPOTIFY_CREATE_PLAYLIST':
                var playlist = state.ui.playlist

                if( !store.getState().spotify.authorized ){
                    alert('Must be logged in to Spotify to do this')
                    return
                }
                store.dispatch( actions.createPlaylist( action.name, action.is_private ))
                break


            case 'SPOTIFY_REMOVE_PLAYLIST_TRACKS':
                var playlist = state.ui.playlist

                if( !store.getState().spotify.authorized ){
                    alert('Must be logged in to Spotify to do this')
                    return
                }
                if( !store.getState().spotify.me || store.getState().spotify.me.id != playlist.owner.id ){
                    alert('You can only modify playlists you own')
                    return
                }
                store.dispatch( actions.deleteTracksFromPlaylist( playlist.uri, playlist.snapshot_id, action.tracks_indexes ))
                break


            case 'SPOTIFY_ADD_PLAYLIST_TRACKS':

                if( !store.getState().spotify.authorized ){
                    alert('Must be logged in to Spotify to do this')
                    return
                }
                store.dispatch( actions.addTracksToPlaylist( action.playlist_uri, action.tracks_uris ))
                break


            case 'SPOTIFY_REORDER_PLAYLIST_TRACKS':

                if( !store.getState().spotify.authorized ){
                    alert('Must be logged in to Spotify to do this')
                    return
                }
                store.dispatch( actions.reorderPlaylistTracks( action.uri, action.indexes, action.to_index ))
                break


            case 'SPOTIFY_SAVE_PLAYLIST':

                if( !store.getState().spotify.authorized ){
                    alert('Must be logged in to Spotify to do this')
                    return
                }
                store.dispatch( actions.savePlaylist( action.uri, action.name, action.is_public ))
                break

            // when our mopidy server current track changes
            case 'MOPIDY_CURRENTTLTRACK':

                // proceed as usual so we don't inhibit default functionality
                next(action)

                // if the current track is a spotify track
                if( action.data && action.data.track.uri.substring(0,14) == 'spotify:track:' ){
                    store.dispatch( actions.getTrack( action.data.track.uri ) )
                }

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default SpotifyMiddleware


var actions = require('./actions.js')
var spotifyActions = require('../spotify/actions.js')
var helpers = require('../../helpers.js')

const UIMiddleware = (function(){

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {
        var state = store.getState();

        switch(action.type){

            case 'REMOVE_TRACKS_FROM_PLAYLIST':
                var playlist = store.getState().ui.playlist
                switch( helpers.uriSource( playlist.uri ) ){

                    case 'spotify':
                        if( !store.getState().spotify.authorized ){
                            alert('Must be logged in to Spotify to do this')
                            return
                        }
                        if( !store.getState().spotify.me || store.getState().spotify.me.id != playlist.owner.id ){
                            alert('You can only modify tracks you own')
                            return
                        }
                        store.dispatch( spotifyActions.deleteTracksFromPlaylist( playlist.uri, playlist.snapshot_id, action.track_indexes ))
                        break

                    case 'm3u':
                        alert('TODO: delete from M3U')
                        break
                }

            case 'ADD_TRACKS_TO_PLAYLIST':
                console.log(action)
                break

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default UIMiddleware
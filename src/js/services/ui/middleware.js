

var uiActions = require('./actions.js')
var spotifyActions = require('../spotify/actions.js')
var helpers = require('../../helpers.js')

const UIMiddleware = (function(){

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {

        switch(action.type){

            case 'CREATE_NOTIFICATION':

                // start a timeout to remove this notification
                var timeout = setTimeout(
                    function(){
                        store.dispatch(uiActions.removeNotification(action.notification.id))
                    },
                    3000
                )

                next(action)
                break

            case 'PLAYLIST_TRACKS_ADDED':

                store.dispatch(uiActions.createNotification('Added '+action.tracks_uris.length+' tracks to playlist'))

                next(action)
                break


            case 'MOPIDY_STATE':
                helpers.setWindowTitle(store.getState().ui.current_track, action.data)
                next(action)
                break

            case 'MOPIDY_CURRENTTLTRACK':
                if( action.data && action.data.track ) helpers.setWindowTitle(action.data.track, store.getState().mopidy.play_state)
                next(action)
                break

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action)
        }
    }

})();

export default UIMiddleware
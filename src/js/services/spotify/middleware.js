
var actions = require('./actions.js')

const SpotifyMiddleware = (function(){

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {
        var state = store.getState();

        switch(action.type){

            // when our mopidy server current track changes
            case 'MOPIDY_CURRENTTLTRACK':
                // DISABLED AS IT CAUSES ISSUE

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
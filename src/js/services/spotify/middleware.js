
import actions from './actions'

const SpotifyMiddleware = (function(){

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {
        return next(action);
        /*
        var state = store.getState();

        switch(action.type){

            case 'SPOTIFY_AUTHORIZATION_GRANTEDXXX':

                // proceed as usual
                next(action)

                // now we've been granted, 
                store.dispatch({ type: 'SPOTIFY_AUTHORIZATION_COMPLETE', data: data })
                break;

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
        */
    }

})();

export default SpotifyMiddleware
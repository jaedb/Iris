
import actions from './actions'

const SpotifyMiddleware = (function(){

    return store => next => action => {
        
        switch(action.type) {

            case 'SPOTIFY_CONNECT':

                console.log('Spotify wants to connect')

                break;

            case 'SPOTIFY_DISCONNECT':
                console.log('Spotify wants to DISconnect')
                break;

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default SpotifyMiddleware
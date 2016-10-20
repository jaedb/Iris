
import actions from './actions'

const SpotifyMiddleware = (function(){

    /**
     * Send an ajax request to the Spotify API
     *
     * @param endpoint string = the url to query (ie /albums/:uri)
     * @param method string
     * @param data mixed = request payload
     * @return ajax promise
     **/
    const sendRequest = (token, endpoint, method = 'GET', data = false) => {
    
        var options = {
            method: method,
            cache: true,
            url: 'https://api.spotify.com/v1/'+endpoint,
            data: data
        };

        if( token ){
            options.headers = {
                Authorization: 'Bearer '+ token
            }
        }

        return $.ajax( options );
    }

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {

        var state = store.getState();

        switch(action.type){

            case 'SPOTIFY_AUTHORIZATION_GRANTED':
                sendRequest( action.data.access_token, 'me' )
                    .then( (response) => {
                        var data = action.data;
                        data.me = response;
                        store.dispatch({ type: 'SPOTIFY_AUTHORIZATION_COMPLETE', data: data })
                    });
                break;

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default SpotifyMiddleware
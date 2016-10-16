
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
    const sendRequest = (endpoint, method = 'GET', data = false) => {
        return $.ajax({
            method: method,
            cache: true,
            url: 'https://api.spotify.com/v1/'+endpoint,
            data: data
        });
    }

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {
        switch(action.type) {


            case 'SPOTIFY_LOAD_ALBUM':

                store.dispatch({ type: 'SPOTIFY_ALBUM_LOADED', data: false })
                var id = action.uri.replace('spotify:album:','');

                sendRequest('albums/'+id)
                    .then( (response) => {                
                        store.dispatch({ type: 'SPOTIFY_ALBUM_LOADED', data: response })
                    });
                break;


            case 'SPOTIFY_LOAD_ARTIST':

                store.dispatch({ type: 'SPOTIFY_ARTIST_LOADED', data: false })
                var id = action.uri.replace('spotify:artist:','');

                sendRequest('artists/'+id)
                    .then( (response) => {                
                        store.dispatch({ type: 'SPOTIFY_ARTIST_LOADED', data: response })
                    });
                break;


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
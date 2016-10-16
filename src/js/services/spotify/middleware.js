
import actions from './actions'

const SpotifyMiddleware = (function(){

    return store => next => action => {

        switch(action.type) {

            case 'SPOTIFY_LOAD_ALBUM':

                // clear the current album
                store.dispatch({ type: 'SPOTIFY_ALBUM_LOADED', data: false })

                var id = action.uri.replace('spotify:album:','');
                $.ajax({
                    method: 'GET',
                    cache: true,
                    url: 'https://api.spotify.com/v1/albums/'+id,
                    success: function(album){
                        store.dispatch({ type: 'SPOTIFY_ALBUM_LOADED', data: album })
                    }
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
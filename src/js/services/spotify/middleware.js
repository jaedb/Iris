
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

            case 'SPOTIFY_LOAD_ALBUM':

                store.dispatch({ type: 'SPOTIFY_ALBUM_LOADED', data: false })
                var id = action.uri.replace('spotify:album:','');

                sendRequest( false, 'albums/'+id )
                    .then( (response) => {
                        for( var i = 0; i < response.tracks.items.length; i++ ){
                            response.tracks.items[i].album = {
                                name: response.name,
                                uri: response.uri
                            }
                        }
                        store.dispatch({ type: 'SPOTIFY_ALBUM_LOADED', data: response })
                    });
                break;


            case 'SPOTIFY_LOAD_ARTIST':

                store.dispatch({ type: 'SPOTIFY_ARTIST_LOADED', data: false })
                store.dispatch({ type: 'SPOTIFY_ARTIST_ALBUMS_LOADED', data: false })

                var id = action.uri.replace('spotify:artist:','');

                sendRequest( false, 'artists/'+id )
                    .then( (response) => {                
                        store.dispatch({ type: 'SPOTIFY_ARTIST_LOADED', data: response })
                    });

                sendRequest( false, 'artists/'+id+'/albums' )
                    .then( (response) => {
                        store.dispatch({ type: 'SPOTIFY_ARTIST_ALBUMS_LOADED', data: response })
                    });

                sendRequest( false, 'artists/'+id+'/top-tracks?country='+state.spotify.country )
                    .then( (response) => {
                        store.dispatch({ type: 'SPOTIFY_ARTIST_TRACKS_LOADED', data: response })
                    });
                break;


            case 'SPOTIFY_LOAD_LIBRARY_ARTISTS':

                store.dispatch({ type: 'SPOTIFY_LIBRARY_ARTISTS_LOADED', data: false })

                sendRequest( state.spotify.access_token, 'me/following?type=artist' )
                    .then( (response) => {                
                        store.dispatch({ type: 'SPOTIFY_LIBRARY_ARTISTS_LOADED', data: response })
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
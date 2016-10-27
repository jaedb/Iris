
const localstorageMiddleware = (function(){

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {
        
        console.log(action, store.getState())

        // proceed as normal first
        // this way, any reducers and middleware do their thing BEFORE we store our new state
        next(action);

        switch( action.type ){

            case 'PUSHER_SET_CONFIG':
                var pusher = {
                    username: action.config.username,
                    port: action.config.port
                };
                localStorage.setItem('pusher', JSON.stringify(pusher));
                break;

            case 'PUSHER_CONNECTED':
                var pusher = JSON.parse( localStorage.getItem('pusher') );
                if( !pusher ) pusher = {};
                Object.assign(
                    pusher,{
                        connectionid: action.connection.connectionid
                    }
                );
                localStorage.setItem('pusher', JSON.stringify(pusher));
                break;

            case 'MOPIDY_SET_CONFIG':
                var mopidy = {
                    host: action.config.host,
                    port: action.config.port
                };
                localStorage.setItem('mopidy', JSON.stringify(mopidy));
                break;

            case 'SPOTIFY_SET_CONFIG':
                var spotify = {
                    country: action.config.country,
                    locale: action.config.locale
                };
                localStorage.setItem('spotify', JSON.stringify(spotify));
                break;

            case 'SPOTIFY_AUTHORIZATION_GRANTED':
                var spotify = JSON.parse( localStorage.getItem('spotify') );
                if( !spotify ) spotify = {};
                Object.assign(
                    spotify,{
                        authorized: true, 
                        access_token: action.data.access_token, 
                        refresh_token: action.data.refresh_token, 
                        token_expiry: action.data.token_expiry
                    }
                );
                localStorage.setItem('spotify', JSON.stringify(spotify));
                break;

            case 'SPOTIFY_TOKEN_REFRESHED':
                var spotify = JSON.parse( localStorage.getItem('spotify') );
                if( !spotify ) spotify = {};
                Object.assign(
                    spotify,{
                        access_token: action.data.access_token,
                        token_expiry: action.data.token_expiry
                    }
                );
                localStorage.setItem('spotify', JSON.stringify(spotify));
                break;
        }
    }

})();

export default localstorageMiddleware
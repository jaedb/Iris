
const localstorageMiddleware = (function(){

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {
        
        console.log(action)

        // proceed as normal first
        // this way, any reducers and middleware do their thing BEFORE we store our new state
        next(action);

        switch( action.type ){

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

            case 'SPOTIFY_AUTHORIZATION_COMPLETE':
                var spotify = JSON.parse( localStorage.getItem('spotify') );
                if( !spotify ) spotify = {};
                console.log(spotify)
                Object.assign(
                    spotify,{
                        authorized: true, 
                        access_token: action.data.access_token, 
                        refresh_token: action.data.refresh_token,
                        me: action.data.me
                    }
                );
                localStorage.setItem('spotify', JSON.stringify(spotify));
                break;
        }
    }

})();

export default localstorageMiddleware
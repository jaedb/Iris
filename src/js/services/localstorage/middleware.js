
const localstorageMiddleware = (function(){

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {

        // proceed as normal first
        // this way, any reducers and middleware do their thing BEFORE we store our new state
        next(action);

        switch( action.type ){

            case 'MOPIDY_SET_CONFIG':
                var mopidy = {
                    host: action.host,
                    port: action.port
                };
                localStorage.setItem('mopidy', JSON.stringify(mopidy));
                break;

            case 'SPOTIFY_COMPLETE_AUTHORIZATION':
                var spotify = {
                    authorized: true, 
                    access_token: action.data.access_token, 
                    refresh_token: action.data.refresh_token
                };
                localStorage.setItem('spotify', JSON.stringify(spotify));
                break;
        }
    }

})();

export default localstorageMiddleware
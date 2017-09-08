
const localstorageMiddleware = (function(){

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {

        // proceed as normal first
        // this way, any reducers and middleware do their thing BEFORE we store our new state
        next(action);

        // append our state to a global variable. This gives us access to debug the store at any point
        window._store = store

        // if debug enabled
        if (store.getState().ui.log_actions){

            var ignored_actions = [
                'START_LOADING',
                'STOP_LOADING'
            ]

            // Show non-ignored actions
            if (!ignored_actions.includes(action.type)){
                console.log(action)
            }
        }

        switch( action.type ){

            case 'PUSHER_CONNECTED':
                var pusher = JSON.parse( localStorage.getItem('pusher') );
                if( !pusher ) pusher = {};
                Object.assign(
                    pusher,{
                        connection_id: action.connection_id
                    }
                );
                localStorage.setItem('pusher', JSON.stringify(pusher));
                break;

            case 'PUSHER_SET_PORT':
                var pusher = JSON.parse( localStorage.getItem('pusher') );
                if( !pusher ) pusher = {};
                Object.assign( pusher, { port: action.port } );
                localStorage.setItem('pusher', JSON.stringify(pusher));
                break;

            case 'PUSHER_USERNAME_CHANGED':
                var stored_pusher = JSON.parse( localStorage.getItem('pusher') )
                var pusher = Object.assign({}, stored_pusher, { username: action.username })
                localStorage.setItem('pusher', JSON.stringify(pusher))
                break;

            case 'MOPIDY_SET_CONFIG':
                var mopidy = {
                    host: action.config.host,
                    port: action.config.port
                };
                localStorage.setItem('mopidy', JSON.stringify(mopidy));
                break;

            case 'MOPIDY_URISCHEMES_FILTERED':
                var mopidy = JSON.parse( localStorage.getItem('mopidy') );
                if( !mopidy ) mopidy = {};
                Object.assign( mopidy, { uri_schemes: action.data });
                localStorage.setItem('mopidy', JSON.stringify(mopidy));
                break;

            case 'SPOTIFY_SET_CONFIG':
                var spotify = JSON.parse( localStorage.getItem('spotify') );
                if( !spotify ) spotify = {};
                Object.assign(
                    spotify,{
                        authentication_provider: action.config.authentication_provider, 
                        country: action.config.country, 
                        locale: action.config.locale
                    }
                );
                localStorage.setItem('spotify', JSON.stringify(spotify));
                break;

            case 'SPOTIFY_IMPORT_AUTHORIZATION':
            case 'SPOTIFY_AUTHORIZATION_GRANTED':
                if (action.authorization !== undefined){
                    var authorization = action.authorization;
                } else if (action.data){
                    var authorization = action.data;
                }
                var spotify = JSON.parse( localStorage.getItem('spotify') );
                spotify = Object.assign(
                    {},
                    (spotify ? spotify : {}),
                    {
                        authorization: authorization,
                        access_token: authorization.access_token, 
                        refresh_token: authorization.refresh_token, 
                        token_expiry: authorization.token_expiry
                    }
                );
                localStorage.setItem('spotify', JSON.stringify(spotify));
                break;

            case 'SPOTIFY_AUTHORIZATION_REVOKED':
                var spotify = JSON.parse( localStorage.getItem('spotify') );
                spotify = Object.assign(
                    {},
                    (spotify ? spotify : {}),
                    {
                        authorization: false, 
                        access_token: false, 
                        refresh_token: false, 
                        token_expiry: false
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
                        token_expiry: action.data.token_expiry,
                        provider: action.provider
                    }
                );
                localStorage.setItem('spotify', JSON.stringify(spotify));
                break;

            case 'SPOTIFY_ME_LOADED':
                var spotify = JSON.parse( localStorage.getItem('spotify') );
                if( !spotify ) spotify = {};
                Object.assign(
                    spotify,
                    { me: action.data }
                );
                localStorage.setItem('spotify', JSON.stringify(spotify));
                break;

            case 'CORE_SET':
                var core = JSON.parse( localStorage.getItem('core') );
                if( !core ) core = {};
                Object.assign( core, action.data );
                localStorage.setItem('core', JSON.stringify(core));
                break

            case 'UI_SET':
                var ui = JSON.parse( localStorage.getItem('ui') );
                if( !ui ) ui = {};
                Object.assign( ui, action.data );
                localStorage.setItem('ui', JSON.stringify(ui));
                break

            case 'SUPPRESS_BROADCAST':
                var ui = JSON.parse(localStorage.getItem('ui'))
                if (!ui) ui = {}

                var suppressed_broadcasts = (typeof(ui.suppressed_broadcasts) !== 'undefined' ? ui.suppressed_broadcasts : [])
                suppressed_broadcasts.push(action.key)

                Object.assign(
                    ui,
                    { suppressed_broadcasts: suppressed_broadcasts }
                );
                localStorage.setItem('ui', JSON.stringify(ui))
                break
        }
    }

})();

export default localstorageMiddleware
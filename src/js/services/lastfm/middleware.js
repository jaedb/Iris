
import ReactGA from 'react-ga'

var helpers = require('./../../helpers')
var lastfmActions = require('./actions')
var uiActions = require('../ui/actions')
var pusherActions = require('../pusher/actions')

const LastfmMiddleware = (function(){

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {
        var state = store.getState();

        switch(action.type){

            case 'LASTFM_USER_LOADED':
                var user = Object.assign(
                    {},
                    action.user,
                    {
                        uri: "lastfm:user:"+action.user.name
                    }
                );
                store.dispatch({
                    type: "USER_LOADED",
                    key: user.uri,
                    user: user
                });
                next(action);
                break;

            case 'MOPIDY_CURRENTTLTRACK':
                if (action.data && action.data.track){
                    if (state.lastfm.session){
                        store.dispatch(lastfmActions.scrobble(action.data.track));
                    }
                }
                next(action);
                break;


            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default LastfmMiddleware
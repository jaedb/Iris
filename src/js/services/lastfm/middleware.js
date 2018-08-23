
var helpers = require('./../../helpers');
var coreActions = require('../core/actions');

const LastfmMiddleware = (function(){

    return store => next => action => {
        switch(action.type){

            case 'LASTFM_ME_LOADED':
                var user = Object.assign(
                    {},
                    action.me,
                    {
                        uri: "lastfm:user:"+action.me.name
                    }
                );
                store.dispatch(coreActions.userLoaded(user));
                break;

            default:
                return next(action);
        }
    }
})();

export default LastfmMiddleware

var helpers = require('./../../helpers');
var coreActions = require('../core/actions');

const LastfmMiddleware = (function(){

    return store => next => action => {
        switch(action.type){

            case 'LASTFM_ME_LOADED':
                var me = helpers.formatUser(action.me);
                Object.assign(
                    me,
                    {
                        uri: "lastfm:user:"+me.name
                    }
                );
                store.dispatch(coreActions.userLoaded(me));
                action.me = me;
                next(action);
                break;

            default:
                return next(action);
        }
    }
})();

export default LastfmMiddleware
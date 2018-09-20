
var helpers = require('./../../helpers')
var coreActions = require('../core/actions')

const GeniusMiddleware = (function(){
    return store => next => action => {
        var state = store.getState();

        switch(action.type){

            case 'GENIUS_ME_LOADED':
                var me = helpers.formatUser(action.me);                
                Object.assign(
                    me,
                    {
                        uri: "genius:user:"+me.id
                    }
                );

            	store.dispatch({
            		type: 'GENIUS_USER_LOADED',
            		user: action.me
            	});
                action.me = me;
            	next(action);
            	break;

            case 'GENIUS_USER_LOADED':
                var user = helpers.formatUser(action.user);
                Object.assign(
                    user,
                    {
                        uri: "genius:user:"+user.id
                    }
                );
                store.dispatch(coreActions.userLoaded(user));
                action.user = user;
                next(action);
                break;


            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }
})();

export default GeniusMiddleware
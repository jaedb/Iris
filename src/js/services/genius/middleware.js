
var helpers = require('./../../helpers')
var coreActions = require('../core/actions')

const GeniusMiddleware = (function(){
    return store => next => action => {
        var state = store.getState();

        switch(action.type){

            case 'GENIUS_ME_LOADED':
            	store.dispatch({
            		type: 'GENIUS_USER_LOADED',
            		user: action.me
            	});
            	next(action);
            	break;

            case 'GENIUS_USER_LOADED':
                var user = Object.assign(
                    {},
                    action.user,
                    {
                        uri: "genius:user:"+action.user.id
                    }
                );
                store.dispatch(coreActions.userLoaded(user));
                next(action);
                break;


            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }
})();

export default GeniusMiddleware

import ReactGA from 'react-ga'

var helpers = require('../../helpers');
var coreActions = require('../core/actions');
var mopidyActions = require('../mopidy/actions');
var uiActions = require('../ui/actions');

const GoogleMiddleware = (function(){ 

	// A snapcast request is an alias of the Pusher request
    const request = (store, params = null, response_callback = null, error_callback = null) => {
        store.dispatch(
        	pusherActions.request(
        		'snapcast_instruct',
        		params,
        		response_callback,
        		error_callback
        	)
        );
        return;
    }

    return store => next => action => {
        var google = store.getState().google;

        switch(action.type){


            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default GoogleMiddleware



var actions = require('./actions.js')
var spotifyActions = require('../spotify/actions.js')
var helpers = require('../../helpers.js')

const UIMiddleware = (function(){

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {
        var state = store.getState();

        switch(action.type){

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default UIMiddleware
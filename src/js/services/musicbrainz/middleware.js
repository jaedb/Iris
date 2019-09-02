
var helpers = require('./../../helpers');
var musicbrainzActions = require('./actions');

const MusicbrainzMiddleware = (function(){

    return store => next => action => {
        switch(action.type){
            default:
                return next(action);
        }
    }
})();

export default MusicbrainzMiddleware
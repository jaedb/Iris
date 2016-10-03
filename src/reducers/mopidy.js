
import * as actions from '../actions/mopidy'

export default function reducer(mopidy = {}, action){
	console.log(action);
    switch (action.type) {

        case actions.STATUS_CHANGED:
            return Object.assign({}, mopidy, {
            	online: action.online	
            });

        case actions.UPDATE_TRACKLIST:
            return Object.assign({}, mopidy, {
            	tracks: action.tracks	
            });

        case actions.VOLUME_CHANGED:
            return Object.assign({}, mopidy, {
            	volume: action.volume	
            });

        default:
            return mopidy
    }
}




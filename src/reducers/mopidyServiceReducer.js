
import * as actions from '../actions/mopidyServiceActions'

export default function reducer(mopidyService = {}, action){
    switch (action.type) {

        case actions.UPDATE_TRACKLIST:
            return Object.assign({}, mopidyService, {
            	tracks: action.tracks	
            });

        default:
            return mopidyService
    }
}




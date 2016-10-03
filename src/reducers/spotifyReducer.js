
import * as actions from './actions'

export default function reducer(state = {}, action){
    switch (action.type) {

        case actions.AUTHORIZE_SPOTIFY:
            return Object.assign({}, state.spotify, {
            	authorizing: action.authorize	
            });

        case actions.AUTHORIZE_SPOTIFY_SUCCESS:
            console.info('Spotify authorization successful');
            var state = Object.assign({}, state.spotify, action.authorization)
            state = Object.assign({}, state.spotify, { online: true, authorizingSpotify: false })
            return state

        case actions.MOPIDY_ONLINE:
            return Object.assign({}, state.mopidy, {
                online: action.online    
            });

        default:
            return state
    }
}




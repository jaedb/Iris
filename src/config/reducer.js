
import * as actions from './actions'

export default function config(config = {}, action){
    switch (action.type) {

        case actions.AUTHORIZE_SPOTIFY:
            return Object.assign({}, config, {
            	authorizingSpotify: action.authorize	
            });

        case actions.AUTHORIZE_SPOTIFY_SUCCESS:
			console.info('Spotify authorization successful');
            var config = Object.assign({}, config, action.authorization)
            config = Object.assign({}, config, { authorizingSpotify: false })
            return config

        default:
            return config
    }
}




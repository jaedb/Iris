
import * as actions from './actions'

export default function album(album = {}, action) {
    switch (action.type) {

        case actions.SET_ALBUM:
            return Object.assign({}, album, action.album);

        default:
            return album
    }
}




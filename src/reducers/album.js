
import * as actions from '../actions/albumActions'

export default function album(album = {}, action) {
    switch (action.type) {

        case actions.LOAD_ALBUM:
            return album//Object.assign({}, album, action.album);

        default:
            return album
    }
}




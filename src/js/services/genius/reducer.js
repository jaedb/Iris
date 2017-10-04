
export default function reducer(genius = {}, action){
    switch (action.type) {

        case 'GENIUS_CONNECT':
        case 'GENIUS_CONNECTING':
            return Object.assign({}, genius, { connected: false, connecting: true });

        case 'GENIUS_CONNECTED':
            return Object.assign({}, genius, { connected: true, connecting: false });

        default:
            return genius
    }
}




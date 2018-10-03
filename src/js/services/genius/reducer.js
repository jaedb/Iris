
export default function reducer(genius = {}, action){
    switch (action.type){

        case 'GENIUS_SET':
            return Object.assign({}, genius, action.data)

        case 'GENIUS_ME_LOADED':
            return Object.assign({}, genius, {
                me: action.me
            })

        case 'GENIUS_AUTHORIZATION_GRANTED':
            return Object.assign({}, genius, {
                authorizing: false,
                authorization: action.data,
                authorization_code: action.data.authorization_code,
                access_token: action.data.access_token
            })

        case 'GENIUS_AUTHORIZATION_REVOKED':
            return Object.assign({}, genius, {
                authorizing: false,
                authorization: null,
                authorization_code: null,
                access_token: null,
                me: null
            });

        case 'GENIUS_IMPORT_AUTHORIZATION':
            return Object.assign({}, genius, {
                authorizing: false,
                authorization: action.authorization,
                authorization_code: action.authorization.authorization_code,
                access_token: action.authorization.access_token,
                me: null
            })

        default:
            return genius
    }
}

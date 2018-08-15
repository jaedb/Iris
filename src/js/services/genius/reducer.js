
export default function reducer(genius = {}, action){
    switch (action.type){

        case 'GENIUS_SET':
            return Object.assign({}, genius, action.data)

        case 'GENIUS_ME_LOADED':
            return Object.assign({}, genius, {user: action.user})

        case 'GENIUS_AUTHORIZATION_GRANTED':
            return Object.assign({}, genius, {
                authorizing: false, 
                authorization_code: action.data.authorization_code,
                access_token: action.data.access_token
            })

        case 'GENIUS_AUTHORIZATION_REVOKED':
            return Object.assign({}, genius, { 
                authorizing: false,
                authorization_code: false,
                access_token: false,
                me: false
            });

        default:
            return genius
    }
}





export default function reducer(google = {}, action){
    switch (action.type){

        case 'GOOGLE_SET':
            return Object.assign({}, google, action.data);

        default:
            return google
    }
}




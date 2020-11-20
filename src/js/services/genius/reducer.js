
export default function reducer(genius = {}, action) {
  switch (action.type) {
    case 'GENIUS_SET':
      return { ...genius, ...action.data };

    case 'GENIUS_ME_LOADED':
      return { ...genius, me: action.me };

    case 'GENIUS_AUTHORIZATION_GRANTED':
      return {
        ...genius,
        authorizing: false,
        authorization: action.data,
        authorization_code: action.data.authorization_code,
        access_token: action.data.access_token,
      };

    case 'GENIUS_AUTHORIZATION_REVOKED':
      return {
        ...genius,
        authorizing: false,
        authorization: undefined,
        authorization_code: undefined,
        access_token: undefined,
        me: undefined,
      };

    case 'GENIUS_IMPORT_AUTHORIZATION':
      return {
        ...genius,
        authorizing: false,
        authorization: action.authorization,
        authorization_code: action.authorization.authorization_code,
        access_token: action.authorization.access_token,
        me: undefined,
      };

    default:
      return genius;
  }
}

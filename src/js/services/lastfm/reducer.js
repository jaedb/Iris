
export default function reducer(lastfm = {}, action) {
  switch (action.type) {
    case 'LASTFM_CONNECT':
    case 'LASTFM_CONNECTING':
      return { ...lastfm, connected: false, connecting: true };

    case 'LASTFM_CONNECTED':
      return { ...lastfm, connected: true, connecting: false };

    case 'LASTFM_SET':
      return { ...lastfm, ...action.data };

    case 'LASTFM_ME_LOADED':
      return { ...lastfm, me: action.me };

    case 'LASTFM_AUTHORIZATION_GRANTED':
      return {
        ...lastfm,
        authorizing: false,
        authorization: action.data.session,
      };

    case 'LASTFM_AUTHORIZATION_REVOKED':
      return {
        ...lastfm,
        authorizing: false,
        authorization: false,
        me: undefined,
      };

    case 'LASTFM_IMPORT_AUTHORIZATION':
      return {
        ...lastfm,
        authorizing: false,
        authorization: action.authorization,
        me: undefined,
      };

    default:
      return lastfm;
  }
}

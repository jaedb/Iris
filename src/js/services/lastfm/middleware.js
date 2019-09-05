
const helpers = require('./../../helpers');
const coreActions = require('../core/actions');
const lastfmActions = require('./actions');

const LastfmMiddleware = (function () {
  return (store) => (next) => (action) => {
    switch (action.type) {
      case 'LASTFM_ME_LOADED':
        var me = helpers.formatUser(action.me);
        Object.assign(
          me,
          {
            uri: `lastfm:user:${me.name}`,
          },
        );
        store.dispatch(coreActions.userLoaded(me));
        action.me = me;
        next(action);
        break;

      case 'LASTFM_IMPORT_AUTHORIZATION':

        // Wait a few moments before we fetch, allowing the import to complete first
        // TODO: Use callbacks for better code accuracy
        setTimeout(() => { store.dispatch(lastfmActions.getMe()); }, 100);

        next(action);
        break;

      default:
        return next(action);
    }
  };
}());

export default LastfmMiddleware;

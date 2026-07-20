const coreActions = require('../core/actions');
const geniusActions = require('./actions');

const GeniusMiddleware = (function () {
  return (store) => (next) => (action) => {
    switch (action.type) {
      case 'GENIUS_ME_LOADED':
        store.dispatch(coreActions.userLoaded(action.me));
        next(action);
        break;

      case 'GENIUS_IMPORT_AUTHORIZATION':

        // Wait a few moments before we fetch, allowing the import to complete first
        // TODO: Use callbacks for better code accuracy
        setTimeout(() => { store.dispatch(geniusActions.getMe()); }, 100);

        next(action);
        break;

        // This action is irrelevant to us, pass it on to the next middleware
      default:
        return next(action);
    }
  };
}());

export default GeniusMiddleware;

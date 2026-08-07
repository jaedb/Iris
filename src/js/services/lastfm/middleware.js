import ReactGA from 'react-ga';
import { formatUser } from '../../util/format';

const coreActions = require('../core/actions');
const uiActions = require('../ui/actions');
const lastfmActions = require('./actions');

const LastfmMiddleware = (function () {
  return (store) => (next) => (action) => {
    switch (action.type) {
      case 'LASTFM_ME_LOADED':
        var me = formatUser(action.me);
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

      case 'LASTFM_AUTHORIZATION_REVOKED':
        if (store.getState().ui.allow_reporting) {
          ReactGA.event({ category: 'LastFM', action: 'Authorization revoked' });
        }

        store.dispatch(uiActions.createNotification({
          content: 'Logout successful',
          description: 'If you have shared your authorization, make sure you revoke your token',
          sticky: true,
          links: [
            {
              url: 'https://www.last.fm/settings/applications',
              text: 'Authorized apps',
              new_window: true,
            },
          ],
        }));

        next(action);
        break;

      default:
        return next(action);
    }
  };
}());

export default LastfmMiddleware;

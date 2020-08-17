import { arrayOf } from '../../util/arrays';
const coreActions = require('../core/actions');
const mopidyActions = require('../mopidy/actions');
const uiActions = require('../ui/actions');

const GoogleMiddleware = (function () {
  // A Google request is an alias of the Mopidy request
  const request = (store, method, params = null, response_callback = null, error_callback = null) => {
    store.dispatch(
      mopidyActions.request(
        method,
        params,
        response_callback,
        error_callback,
      ),
    );
  };

  return (store) => (next) => (action) => {
    switch (action.type) {
      case 'GOOGLE_GET_LIBRARY_ALBUMS':
        var last_run = store.getState().ui.processes.GOOGLE_LIBRARY_ALBUMS_PROCESSOR;

        if (!last_run) {
          request(
            store,
            'library.browse',
            {
              uri: 'gmusic:album',
            },
            (response) => {
              if (response.length <= 0) {
                return;
              }

              const uris = arrayOf('uri', response);
              store.dispatch({
                type: 'GOOGLE_LIBRARY_ALBUMS_LOADED',
                uris,
              });

              // Start our process to load the full album objects
              store.dispatch(uiActions.startProcess(
                'GOOGLE_LIBRARY_ALBUMS_PROCESSOR',
                `Loading ${uris.length} Google albums`,
                {
                  uris,
                  total: uris.length,
                  remaining: uris.length,
                },
              ));
            },
          );
        } else if (last_run.status === 'cancelled') {
          store.dispatch(uiActions.resumeProcess('GOOGLE_LIBRARY_ALBUMS_PROCESSOR'));
        } else if (last_run.status === 'finished') {
          // TODO: do we want to force a refresh?
        }
        break;

      case 'GOOGLE_LIBRARY_ALBUMS_PROCESSOR':
        if (store.getState().ui.processes.GOOGLE_LIBRARY_ALBUMS_PROCESSOR !== undefined) {
          const processor = store.getState().ui.processes.GOOGLE_LIBRARY_ALBUMS_PROCESSOR;

          if (processor.status === 'cancelling') {
            store.dispatch(uiActions.processCancelled('GOOGLE_LIBRARY_ALBUMS_PROCESSOR'));
            return false;
          }
        }

        var uris = Object.assign([], action.data.uris);
        var uris_to_load = uris.splice(0, 50);

        if (uris_to_load.length > 0) {
          store.dispatch(uiActions.updateProcess(
            'GOOGLE_LIBRARY_ALBUMS_PROCESSOR',
            `Loading ${uris.length} Google albums`,
            {
              uris,
              remaining: uris.length,
            },
          ));
          store.dispatch(mopidyActions.getAlbums(uris_to_load, { name: 'GOOGLE_LIBRARY_ALBUMS_PROCESSOR', data: { uris } }));
        } else {
          store.dispatch(uiActions.processFinished('GOOGLE_LIBRARY_ALBUMS_PROCESSOR'));
        }

        break;

      case 'GOOGLE_GET_LIBRARY_ARTISTS':
        request(
          store,
          'library.browse',
          {
            uri: 'gmusic:artist',
          },
          (response) => {
            if (response.length <= 0) {
              return;
            }

            const uris = [];
            for (let i = 0; i < response.length; i++) {
              // Convert local URI to actual artist URI
              // See https://github.com/mopidy/mopidy-local-sqlite/issues/39
              response[i].uri = response[i].uri.replace('local:directory?albumartist=', '');
              uris.push(response[i].uri);
            }

            store.dispatch(coreActions.artistsLoaded(response));

            store.dispatch({
              type: 'GOOGLE_LIBRARY_ARTISTS_LOADED',
              uris,
            });
          },
          (error) => {
            store.dispatch(coreActions.handleException(
              'Could not load Google artists',
              error,
            ));
          },
        );
        break;


        // This action is irrelevant to us, pass it on to the next middleware
      default:
        return next(action);
    }
  };
}());

export default GoogleMiddleware;

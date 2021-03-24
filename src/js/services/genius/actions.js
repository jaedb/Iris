import { generateGuid } from '../../util/helpers';
import { makeItemSelector } from '../../util/selectors';
import { formatUser } from '../../util/format';

const coreActions = require('../core/actions');
const uiActions = require('../ui/actions');

export function set(data) {
  return {
    type: 'GENIUS_SET',
    data,
  };
}

/**
 * Send an ajax request to the Genius API
 *
 * @param dispatch obj
 * @param getState obj
 * @param endpoint string = the url to query (ie /song/:id)
 * @param method string
 * @param data mixed = request payload
 * @return Promise
 * */
const sendRequest = (dispatch, getState, endpoint, method = 'GET', data = false) => new Promise((resolve, reject) => {
  if (endpoint.startsWith('https://') || endpoint.startsWith('http://')) {
    var url = endpoint;
  } else {
    var url = `https://api.genius.com/${endpoint}`;
    if (getState().genius.access_token) {
      url += `?access_token=${getState().genius.access_token}`;
    }
  }

  if (data) {
    url += `&${data}`;
  }

  // create our ajax request config
  const config = {
    method,
    url,
    timeout: 30000,
    crossDomain: true,
  };

  // only if we've got data do we add it to the request (this prevents appending of "&false" to the URL)
  if (data) {
    if (typeof (data) === 'string') {
      config.data = data;
    } else {
      config.data = JSON.stringify(data);
    }
  }

  // add reference to loader queue
  const loader_key = generateGuid();
  dispatch(uiActions.startLoading(loader_key, `genius_${endpoint}`));

  function status(response) {
    dispatch(uiActions.stopLoading(loader_key));

    if (response.status >= 200 && response.status < 300) {
      return Promise.resolve(response);
    }
    return Promise.reject(new Error(response.statusText));
  }

  fetch(url, config)
    .then(status)
    .then((response) => response.json())
    .then((data) => {
      const {
        meta: { status },
        response,
      } = data;
      if (status >= 200 && status < 300 && response) {
        resolve(response);
      } else {
        reject({
          config,
          xhr,
          status,
          error,
        });
      }
    })
    .catch((error) => {
      reject(error);
    });
});

/**
 * Handle authorization process
 * */

export function authorizationGranted(data) {
  return {
    type: 'GENIUS_AUTHORIZATION_GRANTED',
    data,
  };
}

export function revokeAuthorization() {
  return {
    type: 'GENIUS_AUTHORIZATION_REVOKED',
  };
}

export function importAuthorization(authorization) {
  return {
    type: 'GENIUS_IMPORT_AUTHORIZATION',
    authorization,
  };
}

/**
 * Get current user
 * */
export function getMe() {
  return (dispatch, getState) => {
    sendRequest(dispatch, getState, 'account')
      .then(
        (response) => {
          const me = formatUser(response.user);
          dispatch({
            type: 'GENIUS_ME_LOADED',
            me: {
              ...me,
              uri: `genius:user:${me.id}`,
            },
          });
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load your Genius profile',
            error,
          ));
        },
      );
  };
}

/**
 * Extract lyrics from a page
 * We don't get the lyrics in the API, so we need to 'scrape' the HTML page instead
 *
 * @param uri = track uri
 * @param path = String, the relative API path for the HTML lyrics
 * */
export function getTrackLyrics(uri, path) {
  return (dispatch, getState) => {
    dispatch(coreActions.itemLoaded({
      uri,
      lyrics: null,
      lyrics_path: null,
    }));

    const url = `//${getState().mopidy.host}:${getState().mopidy.port}/iris/http/get_lyrics?path=${path}&connection_id=${getState().pusher.connection_id}`;
    const config = {
      method: 'GET',
      timeout: 10000,
    };

    // add reference to loader queue
    const loader_key = generateGuid();
    dispatch(uiActions.startLoading(loader_key, `genius_get_lyrics_${uri}`));

    function status(response) {
      dispatch(uiActions.stopLoading(loader_key));

      if (response.status >= 200 && response.status < 300) {
        return Promise.resolve(response);
      }
      return Promise.reject(new Error(response.statusText));
    }

    fetch(url, config)
      .then(status)
      .then((response) => response.json())
      .then((data) => {
        if (data.result) {
          const html = $(data.result);
          let lyrics = html.find('.lyrics');
          if (lyrics.length > 0) {
            lyrics = lyrics.first();
            lyrics.find('a').replaceWith((k, v) => v);

            let lyrics_html = lyrics.html();
            lyrics_html = lyrics_html.replace(/(\[)/g, '<span class="mid_grey-text">[');
            lyrics_html = lyrics_html.replace(/(\])/g, ']</span>');

            // console.debug(lyrics_html);

            dispatch(coreActions.itemLoaded({
              uri,
              lyrics: lyrics_html,
              lyrics_path: path,
            }));
          }
        } else {
          dispatch(coreActions.handleException(
            'Could not get track lyrics',
            data.error,
          ));
        }
      })
      .catch((error) => {
        dispatch(coreActions.handleException(
          'Could not get track lyrics',
          error,
        ));
      });
  };
}

export function findTrackLyrics(uri) {
  return (dispatch, getState) => {
    const selector = makeItemSelector(uri);
    const track = selector(getState());
    if (!track || !track.artists) {
      return;
    }

    const loader_key = generateGuid();
    dispatch(uiActions.startLoading(loader_key, `genius_find_lyrics_${uri}`));

    let query = '';
    query += `${track.artists[0].name} `;
    query += track.name;
    query = query.toLowerCase();
    query = query.replace(/\([^)]*\) */g, ''); // anything in circle-braces
    query = query.replace(/\([^[]*\] */g, ''); // anything in square-braces
    query = query.replace(/(?= - ).*$/g, ''); // remove anything after and including " - "

    sendRequest(dispatch, getState, 'search', 'GET', `q=${encodeURIComponent(query)}`)
      .then(
        (response) => {
          if (response.hits && response.hits.length > 0) {
            const lyrics_results = [];
            for (let i = 0; i < response.hits.length; i++) {
              lyrics_results.push({
                title: response.hits[i].result.full_title,
                url: response.hits[i].result.url,
                path: response.hits[i].result.path,
              });
            }
            dispatch(coreActions.itemLoaded({
              uri: track.uri,
              lyrics_results,
            }));

            // Immediately go and get the first result's lyrics
            const lyrics_result = lyrics_results[0];
            dispatch(getTrackLyrics(track.uri, lyrics_result.path));
          }
          dispatch(uiActions.stopLoading(loader_key));
        },
        (error) => {
          dispatch(uiActions.stopLoading(loader_key));
          dispatch(coreActions.handleException(
            'Could not search for track lyrics',
            error,
          ));
        },
      );
  };
}

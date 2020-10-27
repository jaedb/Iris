
import { generateGuid } from '../../util/helpers';
import { formatArtist } from '../../util/format';

const coreActions = require('../core/actions');
const uiActions = require('../ui/actions');

/**
 * Send an ajax request to the LastFM API
 *
 * @param dispatch = obj
 * @param endpoint = String
 * @param params = String
 * */
const sendRequest = (dispatch, getState, endpoint, params) => new Promise((resolve, reject) => {

  const key = 'CXIwsVMAjrXIVitBWgqd';
  const secret = 'KiEUfwKpebxRnEHlKoXnYIftJxeuqjTK';
  const url = `https://api.discogs.com/${endpoint}?${params}`;

  const loader_key = generateGuid();
  dispatch(uiActions.startLoading(loader_key, `discogs_${endpoint}`));

  const config = {
    method: 'GET',
    timeout: 30000,
    mode: 'cors',
    headers: {
      'User-Agent': 'Iris/1.0',
      'Authorization': `Discogs key=${key}, secret=${secret}`
    },
  };

  function status(response) {
    dispatch(uiActions.stopLoading(loader_key));

    if (response.status >= 200 && response.status < 300) {
      return Promise.resolve(response)
    } else {
      return Promise.reject(new Error(response.statusText))
    }
  }

  fetch(url, config)
    .then(status)
    .then(response => response.json())
    .then(data => {
      resolve(data);
    })
    .catch(error => {
      reject(error);
    });
});

export function getArtistImages(uri, artist) {
  return (dispatch, getState) => {
    sendRequest(dispatch, getState, 'database/search', `type=artist&query=${artist.name}`)
      .then(
        (response) => {
          if (response) {
            if (response.results.length > 0 && response.results[0].cover_image !== undefined) {
              dispatch(coreActions.itemLoaded(formatArtist({
                uri,
                images: [response.results[0].cover_image],
              })));
            } else {
              console.log(`Discogs: No results for artist ${artist.name}`);
            }
          }
        },
        (error) => {
          console.error(error);
        },
      );

    /*
    const finalise = (cover_image, cache) => {
      const updated_artist = {
        uri,
        images: [cover_image],
      };
      dispatch(coreActions.artistLoaded(updated_artist));

      if (cache) {
        helpers.cache.set(`discogs_${artist.name}`, cover_image);
      }
    }

    const cached = helpers.cache.get(`discogs_${artist.name}`);
    if (cached) {
      finalise(cached);
      return;
    }

    sendRequest(dispatch, getState, 'database/search', `type=artist&query=${artist.name}`)
      .then(
        (response) => {
          if (response) {
            if (response.results.length > 0 && response.results[0].cover_image !== undefined) {
              finalise(response.results[0].cover_image, true);
            } else {
              console.log(`Discogs: No results for artist ${artist.name}`);
            }
          }
        },
        (error) => {
          console.error(error);
        },
      );
      */
  };
}

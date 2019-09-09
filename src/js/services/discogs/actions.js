
const coreActions = require('../core/actions');
const uiActions = require('../ui/actions');
const helpers = require('../../helpers');

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

  const loader_key = helpers.generateGuid();
  dispatch(uiActions.startLoading(loader_key, `discogs_${endpoint}`));

  const checkRateLimit = (request) => {    
    const rate = {
      limit: request.getResponseHeader('X-Discogs-Ratelimit'),
      remaining: request.getResponseHeader('X-Discogs-Ratelimit-Remaining'),
      used: request.getResponseHeader('X-Discogs-Ratelimit-Used'),
    }
    if (rate.remaining !== undefined && rate.remaining === 0) {
      dispatch(uiActions.createNotification({
        key: "discogs_rate_limit",
        type: "bad",
        content: "Rate limit exceeded",
        description: `Discogs rate limit exceeded, try again in a few minutes.`
      }));
    }
  }

  const config = {
    method: 'GET',
    cache: true,
    timeout: 30000,
    url: url,
    crossDomain: true,
    headers: {
      'User-Agent': 'Iris/1.0',
      'Authorization': `Discogs key=${key}, secret=${secret}`
    },
  };

  $.ajax(config).then(
    (data, textStatus, request) => {
      dispatch(uiActions.stopLoading(loader_key));
      checkRateLimit(request);
      resolve(data);
    },
    (xhr, status, error) => {
      dispatch(uiActions.stopLoading(loader_key));
      checkRateLimit(xhr);

      reject({
        config,
        error,
        status,
        xhr,
      });
    },
  );
});

export function getArtistImages(uri, artist) {
  return (dispatch, getState) => {
    sendRequest(dispatch, getState, 'database/search', `type=artist&query=${artist.name}`)
      .then(
        (response) => {
          if (response) {
            if (response.results.length > 0 && response.results[0].cover_image !== undefined) {              
              const updated_artist = {
                uri,
                images: [response.results[0].cover_image],
              };
              dispatch(coreActions.artistLoaded(updated_artist));
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

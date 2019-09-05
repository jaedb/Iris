
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
const sendRequest = (dispatch, endpoint, params) => new Promise((resolve, reject) => {
  const loader_key = helpers.generateGuid();

  dispatch(uiActions.startLoading(loader_key, `,discogs_${endpoint}`));

  const key = 'CXIwsVMAjrXIVitBWgqd';
  const secret = 'KiEUfwKpebxRnEHlKoXnYIftJxeuqjTK';

  const config = {
    method: 'GET',
    cache: true,
    timeout: 30000,
    url: `https://api.discogs.com/${endpoint}?key=${key}&secret=${secret}&${params}`,
    headers: {
      'user-agent': 'Iris/1.0',
    },
  };

  $.ajax(config).then(
    (response) => {
      dispatch(uiActions.stopLoading(loader_key));
      if (response.error) {
        reject({
          config,
          error: response,
        });
      } else {
        resolve(response);
      }
    },
    (xhr, status, error) => {
      dispatch(uiActions.stopLoading(loader_key));

      // Snatch a more meaningful error
      let description = null;
      if (xhr && xhr.responseJSON && xhr.responseJSON.message) {
        description = xhr.responseJSON.message;
      }

      reject({
        config,
        error,
        description,
        status,
        xhr,
      });
    },
  );
});

export function findArtist(name) {
  return sendRequest(dispatch, `artists/${artist.mbid}`, 'inc=url-rels')
    .then(
      (response) => {
        if (response) {
          const image_relations = response.relations.filter((rel) => rel.type == 'image');
          if (image_relations) {
            const updated_artist = {
              uri,
              images: image_relations.map((relation) => relation.url.resource.replace('File:', 'Special:FilePath/')),
            };
            dispatch(coreActions.artistLoaded(updated_artist));
          }
        }
      },
      (error) => {
        console.log(`Musicbrainz: No results for ${artist.mbid}`);
      },
    );
}

export function getArtistImages(uri, artist) {
  return (dispatch, getState) => {
    sendRequest(dispatch, 'database/search', `query=${artist.name}`)
      .then(
        (response) => {
          if (response) {
            if (response.results && response.results[0].cover_image) {
              const updated_artist = {
                uri,
                images: [response.results[0].cover_image],
              };
              dispatch(coreActions.artistLoaded(updated_artist));
            }
          }
        },
        (error) => {
          console.log(`Discogs: No results for ${artist.name}`);
        },
      );
  };
}

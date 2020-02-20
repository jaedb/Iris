
const coreActions = require('../core/actions');
const uiActions = require('../ui/actions');
const helpers = require('../../helpers');

export function set(data) {
  return {
    type: 'LASTFM_SET',
    data,
  };
}

/**
 * Send an ajax request to the LastFM API
 *
 * @param dispatch = obj
 * @param getState = obj
 * @param params = string, the url params to send
 * @params signed = boolean, whether we've got a signed request with baked-in api_key
 * */
const sendRequest = (dispatch, getState, params, signed = false) => new Promise((resolve, reject) => {
  let url = `https://ws.audioscrobbler.com/2.0/?format=json&${params}`;
  let http_method = 'GET';
  let method = params.substring(params.indexOf('method=') + 7, params.length);
  method = method.substring(0, method.indexOf('&'));

  // Signed requests don't need our api_key as the proxy has it's own
  if (!signed) {
    url += '&api_key=4320a3ef51c9b3d69de552ac083c55e3';
  } else {
    http_method = 'POST';
  }

  const config = {
    method: http_method,
    timeout: 30000,
  };

  const loader_key = helpers.generateGuid();
  dispatch(uiActions.startLoading(loader_key, `lastfm_${method}`));

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
      resolve(data);
    })
    .catch((error) => {
      reject(error);
    });
});

/**
 * Send a SIGNED ajax request to the LastFM API
 *
 * @param dispatch = obj
 * @param getState = obj
 * @param params = string, the url params to send
 * @param signed = boolean
 * */
const sendSignedRequest = (dispatch, getState, params) => new Promise((resolve, reject) => {
  // Not authorized
  if (!getState().lastfm.authorization) {
    reject({
      params,
      error: 'No active LastFM authorization (session)',
    });
  }

  const loader_key = helpers.generateGuid();
  let method = params.substring(params.indexOf('method=') + 7, params.length);
  method = method.substring(0, method.indexOf('&'));

  dispatch(uiActions.startLoading(loader_key, `lastfm_${method}`));

  params += `&sk=${getState().lastfm.authorization.key}`;
  const url = `${getState().lastfm.authorization_url}?action=sign_request&${params}`;

  const config = {
    method: 'GET',
    timeout: 30000,
  };

  function status(response) {
    dispatch(uiActions.stopLoading(loader_key));

    if (response.status >= 200 && response.status < 300) {
      return Promise.resolve(response)
    }
    return Promise.reject(new Error(response.statusText));
  }

  fetch(url, config)
    .then(status)
    .then((response) => response.json())
    .then((data) => {
      // Now we have signed params, we can make the actual request
      sendRequest(dispatch, getState, data.params, true)
        .then(
          (response) => resolve(response),
          (error) => reject(error),
        );
    })
    .catch((error) => {
      reject(error);
    });
});


/**
 * Handle authorization process
 * */

export function authorizationGranted(data) {
  data.session.expiry = new Date().getTime() + 3600;
  return {
    type: 'LASTFM_AUTHORIZATION_GRANTED',
    data,
  };
}

export function revokeAuthorization() {
  return {
    	type: 'LASTFM_AUTHORIZATION_REVOKED',
  };
}

export function importAuthorization(authorization) {
  return {
    type: 'LASTFM_IMPORT_AUTHORIZATION',
    authorization,
  };
}


/**
 * Non-signed requests
 * */

export function getMe() {
  return (dispatch, getState) => {
    const params = `method=user.getInfo&user=${getState().lastfm.authorization.name}`;
    sendRequest(dispatch, getState, params)
      .then(
        (response) => {
          if (response.user) {
            dispatch({
              type: 'LASTFM_ME_LOADED',
              me: response.user,
            });
          }
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not get your LastFM profile',
            error,
          ));
        },
      );
  };
}

export function getTrack(uri) {
  return (dispatch, getState) => {
    if (getState().core.tracks[uri] !== undefined) {
      var track = getState().core.tracks[uri];
      if (!track.artists) {
        dispatch(coreActions.handleException(
          'Could not get LastFM track',
          {},
          'Track has no artists',
        ));
        return;
      }
    } else {
      dispatch(coreActions.handleException(
        'Could not get LastFM track',
        {},
        'Could not find track in index',
      ));
      return;
    }

    const track_name = track.name;
    const artist_name = encodeURIComponent(track.artists[0].name);
    let params = `method=track.getInfo&track=${track_name}&artist=${artist_name}`;
    if (getState().lastfm.authorization) {
      params += `&username=${getState().lastfm.authorization.name}`;
    }
    sendRequest(dispatch, getState, params)
      .then(
        (response) => {
          if (response.track) {
            const merged_track = {

              uri: track.uri,
              ...response.track,
              ...track,
            };
            dispatch(coreActions.trackLoaded(merged_track));
          }
        },
        (error) => {
          console.info(`LastFM: No results for track '${track_name}' by '${artist_name}'`);
        },
      );
  };
}

export function getArtist(uri, artist, mbid = false) {
  return (dispatch, getState) => {
    if (mbid) {
      var params = `method=artist.getInfo&mbid=${mbid}`;
    } else {
      artist = artist.replace('&', 'and');
      artist = encodeURIComponent(artist);
      var params = `method=artist.getInfo&artist=${artist}`;
    }
    sendRequest(dispatch, getState, params)
      .then(
        (response) => {
          if (response.artist) {
                    	const artist = {
              uri,
              mbid: response.artist.mbid,
              biography: response.artist.bio.content,
              biography_publish_date: response.artist.bio.published,
              biography_link: response.artist.bio.links.link.href,
              listeners: parseInt(response.artist.stats.listeners),
            };

            dispatch(coreActions.artistLoaded(artist));
          }
        },
        (error) => {
          console.info(`LastFM: No results for artist '${artist}'`);
        },
      );
  };
}

export function getAlbum(uri, artist, album, mbid = false) {
  return (dispatch, getState) => {
    if (mbid) {
      var params = `method=album.getInfo&mbid=${mbid}`;
    } else {
      artist = encodeURIComponent(artist);
      album = encodeURIComponent(album);
      var params = `method=album.getInfo&album=${album}&artist=${artist}`;
    }
    sendRequest(dispatch, getState, params)
      .then(
        (response) => {
          if (response.album) {
            const existing_album = getState().core.albums[uri];

                    	const album = {
              uri,
              images: response.album.image,
              listeners: parseInt(response.album.listeners),
              play_count: parseInt(response.album.playcount),
              mbid: response.album.mbid,
              wiki: (response.album.wiki ? response.album.wiki.content : null),
              wiki_publish_date: (response.album.wiki ? response.album.wiki.published : null),
            };

            // If we've already got some of this album and it has images aready, don't use our ones.
            // In *most* cases this existing image will be perfectly suffice. This prevents an ugly
            // flicker when the existing image is replaced by the LastFM one
            if (existing_album && existing_album.images) {
              delete album.images;
            }

            dispatch(coreActions.albumLoaded(album));
          }
        },
        (error) => {
          console.info(`LastFM: No results for album '${album}'`);
        },
      );
  };
}


export function getImages(context, uri) {
  return (dispatch, getState) => {
    let record = getState().core[context][uri];
    if (record) {
      switch (context) {
        case 'tracks':

          if (record.mbid) {
            var params = `method=album.getInfo&mbid=${record.mbid}`;
          } else if (record.artists && record.artists.length > 0 && record.album) {
            var artist = encodeURIComponent(record.artists[0].name);
            var album = encodeURIComponent(record.album.name);
            var params = `method=album.getInfo&album=${album}&artist=${artist}`;
          }

          if (params) {
            sendRequest(dispatch, getState, params)
              .then(
                (response) => {
                  if (response.album) {
                    const images = helpers.formatImages(response.album.image);
                    dispatch(coreActions.trackLoaded({ uri, images }));
                  }
                },
              );
          }
          break;

        case 'albums':

          if (record.mbid) {
            var params = `method=album.getInfo&mbid=${record.mbid}`;
          } else if (record.artists && record.artists.length > 0) {
            var artist = encodeURIComponent(record.artists[0].name);
            var album = encodeURIComponent(record.name);
            var params = `method=album.getInfo&album=${album}&artist=${artist}`;
          }

          if (params) {
            sendRequest(dispatch, getState, params)
              .then(
                (response) => {
                  if (response.album) {
                    record = { ...record, images: response.album.image };
                    dispatch(coreActions.albumLoaded(record));
                  }
                },
              );
          }
          break;
      }
    }
  };
}


/**
 * Signed requests
 * */

export function loveTrack(uri) {
  return (dispatch, getState) => {
    if (getState().core.tracks[uri] !== undefined) {
      var track = getState().core.tracks[uri];
      if (!track.artists) {
        dispatch(coreActions.handleException(
          'Could not love LastFM track',
          track,
          'Track has no artists',
        ));
        return;
      }
    } else {
      dispatch(coreActions.handleException(
        'Could not love LastFM track',
        track,
        'Could not find track in index',
      ));
      return;
    }

    const artist = encodeURIComponent(track.artists[0].name);
    const params = `method=track.love&track=${track.name}&artist=${artist}`;
    sendSignedRequest(dispatch, getState, params)
      .then(
        (response) => {
          track = {

            ...track,
            userloved: true,
          };
          dispatch({
            type: 'TRACKS_LOADED',
            tracks: [track],
          });
        },
      );
  };
}

export function unloveTrack(uri) {
  return (dispatch, getState) => {
    if (getState().core.tracks[uri] !== undefined) {
      var track = getState().core.tracks[uri];
      if (!track.artists) {
        dispatch(coreActions.handleException(
          'Could not unlove LastFM track',
          track,
          'Track has no artists',
        ));
        return;
      }
    } else {
      dispatch(coreActions.handleException(
        'Could not unlove LastFM track',
        track,
        'Could not find track in index',
      ));
      return;
    }

    const artist = encodeURIComponent(track.artists[0].name);
    const params = `method=track.unlove&track=${track.name}&artist=${artist}`;
    sendSignedRequest(dispatch, getState, params)
      .then(
        (response) => {
          track = {

            ...track,
            userloved: false,
          };
          dispatch({
            type: 'TRACKS_LOADED',
            tracks: [track],
          });
        },
      );
  };
}

/**
 * TODO: Currently scrobbling client-side would result in duplicated scrobbles
 * if the user was authorized across multiple connections. Ideally this would
 * be handled server-side. Mopidy-Scrobbler currently achieves this.
 * */
export function scrobble(track) {
  return (dispatch, getState) => {
    const track_name = track.name;
    var artist_name = 'Unknown';
    if (track.artists) {
      artist_name = track.artists[0].name;
    }
    var artist_name = encodeURIComponent(artist_name);

    let params = 'method=track.scrobble';
    params += `&track=${track_name}&artist=${artist_name}`;
    params += `&timestamp=${Math.floor(Date.now() / 1000)}`;

    sendSignedRequest(dispatch, getState, params)
      .then(
        (response) => {
          console.log('Scrobbled', response);
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not scrobble LastFM track',
            error,
            (error.description ? error.description : null),
          ));
        },
      );
  };
}

import React from 'react';
import {
  collate,
  formatImages,
  formatTrack,
  formatArtist,
  formatAlbum,
} from '../../util/format';
import { generateGuid } from '../../util/helpers';
import { makeItemSelector, getItem } from '../../util/selectors';
import URILink from '../../components/URILink';

const coreActions = require('../core/actions');
const uiActions = require('../ui/actions');

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

  const loader_key = generateGuid();
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

  const loader_key = generateGuid();
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
    const selector = makeItemSelector(uri);
    const track = selector(getState());
    if (!track || !track.artists) {
      dispatch(coreActions.handleException(
        'Could not get LastFM track',
        {},
        'Not in index or has no artists',
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
            dispatch(
              coreActions.itemLoaded(
                formatTrack({
                  uri: track.uri,
                  ...response.track,
                  ...track,
                }),
              ),
            );
          }
        },
      );
  };
}

export function getArtist(uri, name, mbid = false) {
  return (dispatch, getState) => {
    if (mbid) {
      var params = `method=artist.getInfo&mbid=${mbid}`;
    } else {
      name = name.replace('&', 'and');
      name = encodeURIComponent(name);
      var params = `method=artist.getInfo&artist=${name}`;
    }
    sendRequest(dispatch, getState, params)
      .then(
        (response) => {
          if (response.artist) {
            dispatch(
              coreActions.itemLoaded(
                formatArtist({
                  uri,
                  mbid: response.artist.mbid,
                  biography: response.artist.bio.content,
                  biography_publish_date: response.artist.bio.published,
                  biography_link: response.artist.bio.links.link.href,
                  listeners: parseInt(response.artist.stats.listeners),
                }),
              ),
            );
          }
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
            const existing_album = getState().core.items[uri];
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

            dispatch(coreActions.itemLoaded(formatAlbum(album)));
          }
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
          } else {
            record = collate(record, { artists: getState().core.artists });
            if (record.artists && record.artists.length > 0 && record.album) {
              var artist = encodeURIComponent(record.artists[0].name);
              var album = encodeURIComponent(record.album.name);
              var params = `method=album.getInfo&album=${album}&artist=${artist}`;
            }
          }

          if (params) {
            sendRequest(dispatch, getState, params)
              .then(
                (response) => {
                  if (response.album) {
                    const images = formatImages(response.album.image);
                    dispatch(coreActions.itemLoaded(formatAlbum({ uri, images })));
                  }
                },
              );
          }
          break;

        case 'albums':

          if (record.mbid) {
            var params = `method=album.getInfo&mbid=${record.mbid}`;
          } else {
            record = collate(record, { artists: getState().core.artists });
            if (record.artists && record.artists.length > 0) {
              var artist = encodeURIComponent(record.artists[0].name);
              var album = encodeURIComponent(record.name);
              var params = `method=album.getInfo&album=${album}&artist=${artist}`;
            }
          }

          if (params) {
            sendRequest(dispatch, getState, params)
              .then(
                (response) => {
                  if (response.album) {
                    dispatch(
                      coreActions.itemLoaded(formatAlbum({ uri, images: response.album.image })),
                    );
                  }
                },
              );
          }
          break;

        default:
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
    const asset = getItem(getState(), uri) || {};
    if (!asset) {
      dispatch(coreActions.handleException(
        'Could not love LastFM track',
        asset,
        'Could not find track in index',
      ));
      return;
    }
    if (asset && !asset.artists) {
      dispatch(coreActions.handleException(
        'Could not love LastFM track',
        asset,
        'Track has no artists',
      ));
      return;
    }

    const artist = encodeURIComponent(asset.artists[0].name);
    const params = `method=track.love&track=${asset.name}&artist=${artist}`;
    sendSignedRequest(dispatch, getState, params)
      .then(
        () => {
          dispatch(coreActions.itemLoaded({
            uri,
            userloved: true,
          }));
          dispatch(uiActions.createNotification({
            content: <span>Loved <URILink type="track" uri={uri}>{asset ? asset.name : type}</URILink></span>,
          }));
        },
      );
  };
}

export function unloveTrack(uri) {
  return (dispatch, getState) => {
    const asset = getItem(getState(), uri) || {};
    if (!asset) {
      dispatch(coreActions.handleException(
        'Could not love LastFM track',
        asset,
        'Could not find track in index',
      ));
      return;
    }
    if (asset && !asset.artists) {
      dispatch(coreActions.handleException(
        'Could not love LastFM track',
        asset,
        'Track has no artists',
      ));
      return;
    }

    const artist = encodeURIComponent(asset.artists[0].name);
    const params = `method=track.unlove&track=${asset.name}&artist=${artist}`;
    sendSignedRequest(dispatch, getState, params)
      .then(
        () => {
          dispatch(coreActions.itemLoaded({
            uri,
            userloved: false,
          }));
          dispatch(uiActions.createNotification({
            content: <span>Unloved <URILink uri={uri}>{asset ? asset.name : type}</URILink></span>,
          }));
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

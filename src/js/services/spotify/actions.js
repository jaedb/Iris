
import React from 'react';
import {
  arrayOf,
  shuffle,
} from '../../util/arrays';
import {
  generateGuid,
  getFromUri,
  uriType,
  upgradeSpotifyPlaylistUris,
} from '../../util/helpers';
import {
  formatTracks,
  formatPlaylist,
  formatUser,
  formatAlbum,
} from '../../util/format';
import URILink from '../../components/URILink';
import { i18n } from '../../locale';

const coreActions = require('../../services/core/actions');
const uiActions = require('../../services/ui/actions');
const mopidyActions = require('../../services/mopidy/actions');
const lastfmActions = require('../../services/lastfm/actions');

/**
 * Send an ajax request to the Spotify API
 *
 * @param dispatch obj
 * @param getState obj
 * @param endpoint string = the url to query (ie /albums/:uri)
 * @param method string
 * @param data mixed = request payload
 * @param cache boolean
 * @return Promise
 * */
const request = (dispatch, getState, endpoint, method = 'GET', data = false, cache = false) => {
  // Add reference to loader queue
  // We do this straight away so that even if we're refreshing the token, it still registers as
  // loading said endpoint
  const loader_key = generateGuid();
  dispatch(uiActions.startLoading(loader_key, `spotify_${endpoint}`));

  return new Promise((resolve, reject) => {
    getToken(dispatch, getState)
      .then(
        (response) => {
          // prepend the API baseurl, unless the endpoint already has it (ie pagination requests)
          let url = `https://api.spotify.com/v1/${endpoint}`;
          if (endpoint.startsWith('https://api.spotify.com/')) {
            url = endpoint;
          }

          // create our ajax request config
          const config = {
            method,
            timeout: 30000,
            headers: {
              Authorization: `Bearer ${response}`,
              Accept: 'application/json',
            },
          };

          // only if we've got data do we add it to the request (this prevents appending of "&false" to the URL)
          if (data) {
            if (typeof (data) === 'string') {
              config.body = data;
            } else {
              config.body = JSON.stringify(data);
            }
          }

          function status(response) {
            dispatch(uiActions.stopLoading(loader_key));

            // TODO: Rate limiting
            if (response.status === 429) {
              console.error('You hit the Spotify API rate limiter');
            }

            return response.text().then((text) => (text ? JSON.parse(text) : {}));
          }

          fetch(url, config)
            .then(status)
            .then(data => {
              // TODO: Instead of allowing request to fail before renewing the token, once refreshed
              // we should retry the original request(s)
              if (data && data.error) {
                if (data.error.message === 'The access token expired') {
                  dispatch(refreshToken(dispatch, getState));
                } else {
                  reject(data);
                }
              }

              resolve(data);
            })
            .catch(error => {
              reject(error);
            });
        },
        (error) => {
          dispatch(coreActions.handleException(
            error.error,
          ));

          reject(error);
        },
      );
  });
};


/**
* Check an access token validity
*
* @return Promise
* */
function getToken(dispatch, getState) {
  return new Promise((resolve, reject) => {
    // token is okay for now, so just resolve with the current token
    if (getState().spotify.token_expiry && new Date().getTime() < getState().spotify.token_expiry) {
      resolve(getState().spotify.access_token);
      return;
    }

    // token is expiring/expired, so go get a new one and resolve that
    // TODO: Detect whether we already have a pending refresh, in which case we
    // need to wait until it's done, and then return that

    // We've already got a refresh in progress
    if (getState().ui.load_queue.spotify_refresh_token !== undefined) {
      console.log("Already refreshing token, we'll wait 1000ms and try again");

      // Re-check the queue periodically to see if it's finished yet
      // TODO: Look at properly hooking up with the ajax finish event
      setTimeout(
        () =>
          // Return myself for a re-check
          getToken(dispatch, getState),
        1000,
      );
    } else {
      refreshToken(dispatch, getState)
        .then(
          (response) => {
            resolve(response.access_token);
          },
          (error) => {
            reject(error);
          },
        );
    }
  });
}

function refreshToken(dispatch, getState) {
  return new Promise((resolve, reject) => {
    // add reference to loader queue
    const loader_key = generateGuid();
    dispatch(uiActions.startLoading(loader_key, 'spotify_refresh_token'));

    // Fully-authorized, so we can use the local Spotify credentials
    if (getState().spotify.authorization) {
      var config = {
        method: 'GET',
        url: `${getState().spotify.authorization_url}?action=refresh&refresh_token=${getState().spotify.refresh_token}`,
        dataType: 'json',
        timeout: 10000,
      };

      $.ajax(config)
        .then(
          (response) => {
            dispatch(uiActions.stopLoading(loader_key));

            response.token_expiry = new Date().getTime() + (response.expires_in * 1000);
            response.source = 'spotify';
            dispatch({
              type: 'SPOTIFY_TOKEN_REFRESHED',
              data: response,
            });
            resolve(response);
          },
          (xhr, status, error) => {
            dispatch(uiActions.stopLoading(loader_key));

            reject({
              config,
              xhr,
              status,
              error,
            });
          },
        );

      // Server-side authorized (with limited scope) so we need to refresh
      // using the Mopidy-Spotify credentials
    } else {
      var config = {
        method: 'GET',
        url: `//${getState().mopidy.host}:${getState().mopidy.port}/iris/http/refresh_spotify_token`,
        dataType: 'json',
        timeout: 10000,
      };

      $.ajax(config)
        .then(
          (response, status, xhr) => {
            dispatch(uiActions.stopLoading(loader_key));

            if (response.error) {
              reject({
                config,
                xhr,
                status,
                error: response.error,
              });
            } else {
              const token = response.result.spotify_token;
              token.token_expiry = new Date().getTime() + (token.expires_in * 1000);
              token.source = 'mopidy';
              dispatch({
                type: 'SPOTIFY_TOKEN_REFRESHED',
                access_token_provider: 'backend',
                data: token,
              });
              resolve(token);
            }
          },
          (xhr, status, error) => {
            dispatch(uiActions.stopLoading(loader_key));

            reject({
              config,
              xhr,
              status,
              error,
            });
          },
        );
    }
  });
}

export function set(data) {
  return {
    type: 'SPOTIFY_SET',
    data,
  };
}

export function connect() {
  return (dispatch, getState) => {
    dispatch({ type: 'SPOTIFY_CONNECTING' });
    dispatch(getMe());
  };
}


/**
 * Handle authorization process
 * */

export function authorizationGranted(data) {
  data.token_expiry = new Date().getTime() + data.expires_in;
  return { type: 'SPOTIFY_AUTHORIZATION_GRANTED', data };
}

export function revokeAuthorization() {
  return { type: 'SPOTIFY_AUTHORIZATION_REVOKED' };
}

export function refreshingToken() {
  return (dispatch, getState) => {
    dispatch({ type: 'SPOTIFY_TOKEN_REFRESHING' });
    refreshToken(dispatch, getState);
  };
}

export function tokenChanged(spotify_token) {
  return {
    type: 'SPOTIFY_TOKEN_CHANGED',
    spotify_token,
  };
}

export function importAuthorization(authorization) {
  return {
    type: 'SPOTIFY_IMPORT_AUTHORIZATION',
    authorization,
  };
}


/**
 * Get current user
 * */
export function getMe() {
  return (dispatch, getState) => {
    request(dispatch, getState, 'me')
      .then(
        (response) => {
          dispatch({
            type: 'SPOTIFY_ME_LOADED',
            me: response,
          });
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load your profile',
            error,
          ));
        },
      );
  };
}


/**
 * Get a single track
 *
 * @param uri string
 * */
export function getTrack(uri) {
  return (dispatch, getState) => {
    request(dispatch, getState, `tracks/${getFromUri('trackid', uri)}`)
      .then(
        (response) => {
          dispatch(coreActions.trackLoaded(response));
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load track',
            error,
          ));
        },
      );
  };
}

export function getLibraryTracks() {
  return (dispatch, getState) => {
    request(dispatch, getState, 'me/tracks?limit=50')
      .then(
        (response) => {
          dispatch({
            type: 'SPOTIFY_LIBRARY_TRACKS_LOADED',
            data: response,
          });
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not get library tracks',
            error,
          ));
        },
      );
  };
}

export function getFeaturedPlaylists() {
  return (dispatch, getState) => {
    dispatch({ type: 'SPOTIFY_FEATURED_PLAYLISTS_LOADED', data: false });

    const date = new Date();
    date.setHours(date.getHours());
    const year = date.getFullYear();
    let month = date.getMonth();
    if (month < 10) month = `0${month}`;
    let day = date.getDay();
    if (day < 10) day = `0${day}`;
    let hour = date.getHours();
    if (hour < 10) hour = `0${hour}`;
    let min = date.getMinutes();
    if (min < 10) min = `0${min}`;
    let sec = date.getSeconds();
    if (sec < 10) sec = `0${sec}`;

    const timestamp = `${year}-${month}-${day}T${hour}:${min}:${sec}`;

    request(dispatch, getState, `browse/featured-playlists?limit=50&country=${getState().spotify.country}&locale=${getState().spotify.locale}timestamp=${timestamp}`)
      .then(
        (response) => {
          const playlists = [];
          for (let i = 0; i < response.playlists.items.length; i++) {
            playlists.push({

              ...response.playlists.items[i],
              is_completely_loaded: false,
              can_edit: (getState().spotify.me && response.playlists.items[i].owner.id == getState().spotify.me.id),
              tracks_total: response.playlists.items[i].tracks.total,
            });
          }

          dispatch({
            type: 'PLAYLISTS_LOADED',
            playlists,
          });

          dispatch({
            type: 'SPOTIFY_FEATURED_PLAYLISTS_LOADED',
            data: {
              message: response.message,
              playlists: upgradeSpotifyPlaylistUris(arrayOf('uri', response.playlists.items)),
            },
          });
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load featured playlists',
            error,
          ));
        },
      );
  };
}

export function getCategories() {
  return (dispatch, getState) => {
    request(dispatch, getState, `browse/categories?limit=50&country=${getState().spotify.country}&locale=${getState().spotify.locale}`)
      .then(
        (response) => {
          dispatch({
            type: 'SPOTIFY_CATEGORIES_LOADED',
            categories: response.categories.items,
          });
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load categories',
            error,
          ));
        },
      );
  };
}

export function getCategory(id) {
  return (dispatch, getState) => {
    request(dispatch, getState, `browse/categories/${id}?country=${getState().spotify.country}&locale=${getState().spotify.locale}`)
      .then(
        (response) => {
          dispatch({
            type: 'SPOTIFY_CATEGORY_LOADED',
            category: {
              uri: `category:${response.id}`,
              playlist_uris: null,
              ...response,
            },
          });
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load category',
            error,
          ));
        },
      );
  };
}

export function getCategoryPlaylists(id) {
  return (dispatch, getState) => {
    request(dispatch, getState, `browse/categories/${id}/playlists?limit=50&country=${getState().spotify.country}&locale=${getState().spotify.locale}`)
      .then(
        (response) => {
          dispatch({
            type: 'SPOTIFY_CATEGORY_PLAYLISTS_LOADED',
            uri: `category:${id}`,
            playlists: response.playlists,
          });
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load category playlists',
            error,
          ));
        },
      );
  };
}

export function getNewReleases() {
  return (dispatch, getState) => {
    request(dispatch, getState, `browse/new-releases?country=${getState().spotify.country}&limit=50`)
      .then(
        (response) => {
          dispatch({
            type: 'SPOTIFY_NEW_RELEASES_LOADED',
            data: response,
          });
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load new releases',
            error,
          ));
        },
      );
  };
}

export function getURL(url, action_name, key = false) {
  return (dispatch, getState) => {
    request(dispatch, getState, url)
      .then(
        (response) => {
          dispatch({
            type: action_name,
            key,
            data: response,
          });
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load URL',
            error,
          ));
        },
      );
  };
}

export function getMore(url, core_action = null, custom_action = null, extra_data = {}) {
  return (dispatch, getState) => {
    request(dispatch, getState, url)
      .then(
        (response) => {
          if (core_action) {
            dispatch(coreActions.loadedMore(
              core_action.parent_type,
              core_action.parent_key,
              core_action.records_type,
              response,
              extra_data,
            ));
          } else if (custom_action) {
            custom_action.data = response;
            dispatch(custom_action);
          } else {
            dispatch(coreActions.handleException(
              'No callback handler for loading more items',
            ));
          }
        },
        (error) => {
          dispatch(coreActions.handleException(
            `Could not load more ${callback_action.parent_type} ${callback_action.records_type}s`,
            error,
          ));
        },
      );
  };
}

export function clearSearchResults() {
  return {
    type: 'SPOTIFY_CLEAR_SEARCH_RESULTS',
  };
}

export function getSearchResults(type, term, limit = 50, offset = 0) {
  return (dispatch, getState) => {
    dispatch(uiActions.startProcess('SPOTIFY_GET_SEARCH_RESULTS_PROCESSOR', 'Searching Spotify'));

    let typeString = type.replace(/s+$/, '');
    if (typeString === 'all') {
      typeString = 'album,artist,playlist,track';
    }

    let url = `search?q=${term}`;
    url += `&type=${typeString}`;
    url += `&country=${getState().spotify.country}`;
    url += `&limit=${limit}`;
    url += `&offset=${offset}`;

    request(dispatch, getState, url)
      .then(
        (response) => {
          if (response.tracks !== undefined) {
            dispatch({
              type: 'SPOTIFY_SEARCH_RESULTS_LOADED',
              context: 'tracks',
              query: { type, term },
              results: formatTracks(response.tracks.items),
              more: response.tracks.next,
            });
          }

          if (response.artists !== undefined) {
            dispatch({
              type: 'ARTISTS_LOADED',
              artists: response.artists.items,
            });
            dispatch({
              type: 'SPOTIFY_SEARCH_RESULTS_LOADED',
              context: 'artists',
              query: { type, term },
              results: arrayOf('uri', response.artists.items),
              more: response.artists.next,
            });
          }

          if (response.albums !== undefined) {
            dispatch({
              type: 'ALBUMS_LOADED',
              albums: response.albums.items,
            });
            dispatch({
              type: 'SPOTIFY_SEARCH_RESULTS_LOADED',
              context: 'albums',
              query: { type, term },
              results: arrayOf('uri', response.albums.items),
              more: response.albums.next,
            });
          }

          if (response.playlists !== undefined) {
            const playlists = response.playlists.items.map((item) => ({
              ...formatPlaylist(item),
              can_edit: (getState().spotify.me && item.owner.id === getState().spotify.me.id),
              tracks_total: item.tracks.total,
            }));
            dispatch({
              type: 'PLAYLISTS_LOADED',
              playlists,
            });

            dispatch({
              type: 'SPOTIFY_SEARCH_RESULTS_LOADED',
              context: 'playlists',
              query: { type, term },
              results: arrayOf('uri', playlists),
              more: response.playlists.next,
            });
          }

          dispatch(uiActions.processFinished('SPOTIFY_GET_SEARCH_RESULTS_PROCESSOR'));
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load search results',
            error,
          ));
        },
      );
  };
}

export function getAutocompleteResults(field_id, query, types = ['album', 'artist', 'playlist', 'track']) {
  return (dispatch, getState) => {
    dispatch({ type: 'SPOTIFY_AUTOCOMPLETE_LOADING', field_id });

    const genre_included = types.includes('genre');
    if (genre_included) {
      const index = types.indexOf('genre');
      types.splice(index, 1);
    }

    let endpoint = `search?q=${query}`;
    endpoint += `&type=${types.join(',')}`;
    endpoint += `&country=${getState().spotify.country}`;

    request(dispatch, getState, endpoint)
      .then(
        (response) => {
          const genres = [];
          if (genre_included) {
            const available_genres = getState().spotify.genres;
            if (available_genres) {
              for (let i = 0; i < available_genres.length; i++) {
                if (available_genres[i].includes(query)) {
                  const genre = available_genres[i];
                  genres.push({
                    name: (genre.charAt(0).toUpperCase() + genre.slice(1)).replace('-', ' '),
                    uri: `spotify:genre:${genre}`,
                  });
                }
              }
            }
          }

          if (response.artists && response.artists.items) {
            dispatch(coreActions.artistsLoaded(response.artists.items));
          }

          if (response.albums && response.albums.items) {
            dispatch(coreActions.albumsLoaded(response.albums.items));
          }

          if (response.playlists && response.playlists.items) {
            dispatch(coreActions.playlistsLoaded(response.playlists.items));
          }

          if (response.tracks && response.tracks.items) {
            dispatch(coreActions.tracksLoaded(response.tracks.items));
          }

          dispatch({
            type: 'SPOTIFY_AUTOCOMPLETE_LOADED',
            field_id,
            results: {
              artists: (response.artists ? response.artists.items : []),
              albums: (response.albums ? response.albums.items : []),
              playlists: (response.playlists ? response.playlists.items : []),
              tracks: (response.tracks ? response.tracks.items : []),
              genres,
            },
          });
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load autocomplete results',
            error,
          ));
        },
      );
  };
}

export function clearAutocompleteResults(field_id = null) {
  return {
    type: 'SPOTIFY_AUTOCOMPLETE_CLEAR',
    field_id,
  };
}

export function following(uri, method = 'GET') {
  return (dispatch, getState) => {
    const asset_name = uriType(uri);
    let endpoint;
    let data;
    let is_following = null;
    const asset = getState().core[`${asset_name}s`] && getState().core[`${asset_name}s`][uri];

    if (method == 'PUT') {
      is_following = true;
    } else if (method == 'DELETE') {
      is_following = false;
    }

    switch (asset_name) {
      case 'track':
        if (method == 'GET') {
          endpoint = `me/tracks/contains?ids=${getFromUri('trackid', uri)}`;
        } else {
          endpoint = `me/tracks?ids=${getFromUri('trackid', uri)}`;
        }
        break;
      case 'album':
        if (method == 'GET') {
          endpoint = `me/albums/contains?ids=${getFromUri('albumid', uri)}`;
        } else {
          endpoint = `me/albums?ids=${getFromUri('albumid', uri)}`;
        }
        break;
      case 'artist':
        if (method == 'GET') {
          endpoint = `me/following/contains?type=artist&ids=${getFromUri('artistid', uri)}`;
        } else {
          endpoint = `me/following?type=artist&ids=${getFromUri('artistid', uri)}`;
          data = {};
        }
        break;
      case 'user':
        if (method == 'GET') {
          endpoint = `me/following/contains?type=user&ids=${getFromUri('userid', uri)}`;
        } else {
          endpoint = `me/following?type=user&ids=${getFromUri('userid', uri)}`;
          data = {};
        }
        break;
      case 'playlist':
        if (method == 'GET') {
          endpoint = `playlists/${getFromUri('playlistid', uri)}/followers/contains?ids=${getState().spotify.me.id}`;
        } else {
          endpoint = `playlists/${getFromUri('playlistid', uri)}/followers?`;
        }
        break;
      default:
        break;
    }

    request(dispatch, getState, endpoint, method, data)
      .then(
        (response) => {
          if (Array.isArray(response) && response.length > 0) {
            is_following = response[0];
          } else {
            is_following = is_following;
          }

          dispatch({
            type: `SPOTIFY_LIBRARY_${asset_name.toUpperCase()}_CHECK`,
            key: uri,
            in_library: is_following,
          });

          if (method === 'DELETE') {
            dispatch(uiActions.createNotification({
              content: <span>Removed <URILink uri={uri}>{asset ? asset.name : asset_name}</URILink> from library</span>,
            }));
          } else if (method === 'PUT' || method === 'POST') {
            dispatch(uiActions.createNotification({
              content: <span>Added <URILink uri={uri}>{asset ? asset.name : asset_name}</URILink> to library</span>,
            }));
          }
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not follow/unfollow',
            error,
          ));
        },
      );
  };
}

/**
 * Resolve radio seeds into full objects
 *
 * @param radio object
 * */
export function resolveRadioSeeds(radio) {
  return (dispatch, getState) => {
    if (radio.seed_artists.length > 0) {
      let artist_ids = '';
      for (var i = 0; i < radio.seed_artists.length; i++) {
        if (i > 0) artist_ids += ',';
        artist_ids += getFromUri('artistid', radio.seed_artists[i]);
      }

      request(dispatch, getState, `artists?ids=${artist_ids}`)
        .then(
          (response) => {
            if (response && response.artists) {
              dispatch({
                type: 'ARTISTS_LOADED',
                artists: response.artists,
              });
            } else {
              console.error('No Spotify artists returned', artist_ids);
            }
          },
          (error) => {
            dispatch(coreActions.handleException(
              'Could not resolve radio artist seeds',
              error,
            ));
          },
        );
    }

    if (radio.seed_tracks.length > 0) {
      let track_ids = '';
      for (var i = 0; i < radio.seed_tracks.length; i++) {
        if (i > 0) track_ids += ',';
        track_ids += getFromUri('trackid', radio.seed_tracks[i]);
      }

      request(dispatch, getState, `tracks?ids=${track_ids}`)
        .then(
          (response) => {
            dispatch({
              type: 'TRACKS_LOADED',
              tracks: response.tracks,
            });
          },
          (error) => {
            dispatch(coreActions.handleException(
              'Could not load radio track seeds',
              error,
            ));
          },
        );
    }
  };
}


/**
 * =============================================================== DISCOVER =============
 * ======================================================================================
 * */


/**
 * Get my favorites
 *
 * @param uri string
 * */
export function getFavorites(limit = 50, term = 'long_term') {
  return (dispatch, getState) => {
    dispatch({ type: 'SPOTIFY_FAVORITES_LOADED', artists: [], tracks: [] });

    $.when(
      request(dispatch, getState, `me/top/artists?limit=${limit}&time_range=${term}`),
      request(dispatch, getState, `me/top/tracks?limit=${limit}&time_range=${term}`),

    ).then(
      (artists_response, tracks_response) => {
        dispatch({
          type: 'SPOTIFY_FAVORITES_LOADED',
          artists: artists_response.items,
          tracks: tracks_response.items,
        });
      },
      (artists_error, tracks_error) => {
        dispatch(coreActions.handleException(
          'Could not load favorites',
          { ...artists_error, ...tracks_error },
        ));
      },
    );
  };
}


/**
 * Get our recommendations
 * This is based off our 'favorites' and then we use those as seeds
 *
 * @param uris = array of artist or track URIs or a genre string
 * */
export function getRecommendations(uris = [], limit = 20, tunabilities = null) {
  return (dispatch, getState) => {
    dispatch({ type: 'CLEAR_SPOTIFY_RECOMMENDATIONS' });

    // build our starting point
    const artists_ids = [];
    const tracks_ids = [];
    const genres = [];

    for (let i = 0; i < uris.length; i++) {
      const uri = uris[i];

      switch (uriType(uri)) {
        case 'artist':
          artists_ids.push(getFromUri('artistid', uri));
          break;

        case 'track':
          tracks_ids.push(getFromUri('trackid', uri));
          break;

        case 'genre':
          genres.push(getFromUri('genreid', uri));
          break;

        case 'default':
          genres.push(uri);
          break;
      }
    }

    // construct our endpoint URL with all the appropriate arguments
    let endpoint = 'recommendations';
    endpoint += `?seed_artists=${artists_ids.join(',')}`;
    endpoint += `&seed_tracks=${tracks_ids.join(',')}`;
    endpoint += `&seed_genres=${genres.join(',')}`;
    endpoint += `&limit=${limit}`;

    if (tunabilities) {
      for (const key in tunabilities) {
        if (tunabilities.hasOwnProperty(key)) {
          endpoint += `&${key}=${tunabilities[key]}`;
        }
      }
    }

    request(dispatch, getState, endpoint)
      .then(
        (response) => {
          const tracks = Object.assign([], response.tracks);

          // We only get simple artist objects, so we need to
          // get the full object. We'll add URIs to our recommendations
          // anyway so we can proceed in the meantime
          const artists_uris = [];
          if (tracks.length > artists_ids.length && tracks.length > 10) {
            while (artists_uris.length < 6) {
              var random_index = Math.round(Math.random() * (tracks.length - 1));
              const artist = tracks[random_index].artists[0];

              // Make sure this artist is not already in our sample, and
              // is not one of the seeds
              if (!artists_uris.includes(artist.uri) && !artists_ids.includes(artist.id)) {
                artists_uris.push(artist.uri);
                dispatch(getArtist(artist.uri));
              }
            }
          }

          // Copy already loaded albums into array
          const albums = [];
          const albums_uris = [];
          if (tracks.length > 10) {
            while (albums.length < 6) {
              var random_index = Math.round(Math.random() * (tracks.length - 1));
              const { album } = tracks[random_index];

              // Make sure this album is not already in our sample
              if (!albums_uris.includes(album.uri)) {
                albums_uris.push(album.uri);
                albums.push(album);
              }
            }
          }

          if (albums.length > 0) {
            dispatch(coreActions.albumsLoaded(albums));
          }

          if (tracks.length > 0) {
            dispatch(coreActions.tracksLoaded(tracks));
          }

          dispatch({
            type: 'SPOTIFY_RECOMMENDATIONS_LOADED',
            seeds_uris: uris,
            tracks_uris: arrayOf('uri', tracks),
            artists_uris,
            albums_uris: arrayOf('uri', albums),
          });
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load recommendations',
            error,
          ));
        },
      );
  };
}


/**
 * Get all the available genres
 *
 * @param uri string
 * */
export function getGenres() {
  return (dispatch, getState) => {
    request(dispatch, getState, 'recommendations/available-genre-seeds')
      .then(
        (response) => {
          dispatch({
            type: 'SPOTIFY_GENRES_LOADED',
            genres: response.genres,
          });
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load genres',
            error,
          ));
        },
      );
  };
}


/**
 * =============================================================== ARTIST(S) ============
 * ======================================================================================
 * */

/**
 * Get a single artist
 *
 * @param uri string
 * @param full boolean (whether we want a full artist object)
 * */
export function getArtist(uri, full = false) {
  return (dispatch, getState) => {
    // Start with an empty object
    // As each requests completes, they'll add to this object
    const artist = {};

    // We need our artist, obviously
    const requests = [
      request(dispatch, getState, `artists/${getFromUri('artistid', uri)}`, 'GET', false, true)
        .then(
          (response) => {
            Object.assign(artist, response);
          },
          (error) => {
            dispatch(coreActions.handleException(
              'Could not load artist',
              error,
            ));
          },
        ),
    ];

    // Do we want a full artist, with all supporting material?
    if (full) {
      requests.push(
        request(dispatch, getState, `artists/${getFromUri('artistid', uri)}/top-tracks?country=${getState().spotify.country}`)
          .then(
            (response) => {
              Object.assign(artist, response);
            },
            (error) => {
              dispatch(coreActions.handleException(
                'Could not load artist\'s top tracks',
                error,
              ));
            },
          ),
      );

      requests.push(
        request(dispatch, getState, `artists/${getFromUri('artistid', uri)}/related-artists`)
          .then(
            (response) => {
              dispatch(coreActions.artistsLoaded(response.artists));
              Object.assign(artist, { related_artists_uris: arrayOf('uri', response.artists) });
            },
            (error) => {
              dispatch(coreActions.handleException(
                'Could not load artist\'s related artists',
                error,
              ));
            },
          ),
      );
    }

    // Run our requests
    $.when.apply($, requests).then(() => {
      if (artist.musicbrainz_id) {
        dispatch(lastfmActions.getArtist(artist.uri, false, artist.musicbrainz_id));
      } else {
        dispatch(lastfmActions.getArtist(artist.uri, artist.name.replace('&', 'and')));
      }

      dispatch(coreActions.artistLoaded(artist));

      // Now go get our artist albums
      if (full) {
        request(dispatch, getState, `artists/${getFromUri('artistid', uri)}/albums?market=${getState().spotify.country}`)
          .then(
            (response) => {
              dispatch({
                type: 'SPOTIFY_ARTIST_ALBUMS_LOADED',
                artist_uri: uri,
                data: response,
              });
            },
            (error) => {
              dispatch(coreActions.handleException(
                'Could not load artist\'s albums',
                error,
              ));
            },
          );
      }
    });
  };
}

export function getArtists(uris) {
  return (dispatch, getState) => {
    // now get all the artists for this album (full objects)
    let ids = '';
    for (let i = 0; i < uris.length; i++) {
      if (ids != '') ids += ',';
      ids += getFromUri('artistid', uris[i]);
    }

    request(dispatch, getState, `artists/?ids=${ids}`)
      .then(
        (response) => {
          for (var i = i; i < response.length; i++) {
            const artist = response;
            for (var i = 0; i < artist.albums.length; i++) {
              dispatch({
                type: 'ALBUM_LOADED',
                album: artist.albums[i],
              });
            }
            artist.albums_uris = arrayOf('uri', artist.albums);
            artist.albums_more = artist.albums.next;
            dispatch(coreActions.artistLoaded(artist));
          }
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load artists',
            error,
          ));
        },
      );
  };
}

// Used to get images for non-Spotify artists
export function getArtistImages(artist) {
  return (dispatch, getState) => {
    request(dispatch, getState, `search?q=${artist.name}&type=artist`)
      .then(response => {
        if (response.artists.items.length > 0) {
          const updatedArtist = {
            uri: artist.uri,
            images: response.artists.items[0].images,
          }
          dispatch(coreActions.artistLoaded(updatedArtist));
        }
      },
        error => {
          dispatch(coreActions.handleException(
            'Could not load artists',
            error,
          ));
        },
      );
  };
}


export function playArtistTopTracks(uri) {
  return (dispatch, getState) => {
    const { artists } = getState().core;

    // Do we have this artist (and their tracks) in our index already?
    if (typeof (artists[uri]) !== 'undefined' && typeof (artists[uri].tracks) !== 'undefined') {
      const uris = arrayOf('uri', artists[uri].tracks);
      dispatch(mopidyActions.playURIs(uris, uri));

      // We need to load the artist's top tracks first
    } else {
      request(dispatch, getState, `artists/${getFromUri('artistid', uri)}/top-tracks?country=${getState().spotify.country}`)
        .then(
          (response) => {
            const uris = arrayOf('uri', response.tracks);
            dispatch(mopidyActions.playURIs(uris, uri));
          },
          (error) => {
            dispatch(coreActions.handleException(
              'Could not play artist\'s top tracks',
              error,
            ));
          },
        );
    }
  };
}


/**
 * =============================================================== USER(S) ==============
 * ======================================================================================
 * */

export function getUser(uri) {
  return (dispatch, getState) => {
    request(dispatch, getState, `users/${getFromUri('userid', uri)}`)
      .then(
        (response) => {
          dispatch(coreActions.userLoaded(formatUser(response)));
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load user',
            error,
          ));
        },
      );
  };
}

export function getUserPlaylists(uri) {
  return (dispatch, getState) => {
    // get the first page of playlists
    request(dispatch, getState, `users/${getFromUri('userid', uri)}/playlists?limit=40`)
      .then(
        (response) => {
          const playlists = [];
          for (const raw_playlist of response.items) {
            let can_edit = false;
            if (getState().spotify.me && raw_playlist.owner.id == getState().spotify.me.id) {
              can_edit = true;
            }

            const playlist = {

              ...formatPlaylist(raw_playlist),
              can_edit,
              tracks_total: raw_playlist.tracks.total,
            };

            playlists.push(playlist);
          }

          dispatch(coreActions.userPlaylistsLoaded(uri, playlists, response.next, response.total));
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load user\'s playlists',
            error,
          ));
        },
      );
  };
}


/**
 * =============================================================== ALBUM(S) =============
 * ======================================================================================
 * */

/**
 * Single album
 *
 * @oaram uri string
 * */
export function getAlbum(uri) {
  return (dispatch, getState) => {
    // get the album
    request(dispatch, getState, `albums/${getFromUri('albumid', uri)}`)
      .then(
        (response) => {
          // dispatch our loaded artists (simple objects)
          dispatch(coreActions.artistsLoaded(response.artists));

          const tracks = Object.assign([], response.tracks.items);

          const album = {
            ...formatAlbum(response),
            artists_uris: arrayOf('uri', response.artists),
            tracks_uris: arrayOf('uri', tracks),
            tracks_more: response.tracks.next,
            tracks_total: response.tracks.total,
          };

          // add our album to all the tracks
          for (var i = 0; i < tracks.length; i++) {
            tracks[i].album = {
              name: album.name,
              uri: album.uri,
            };
          }

          dispatch(coreActions.albumLoaded(album));
          dispatch(coreActions.tracksLoaded(tracks));

          // now get all the artists for this album (full objects)
          // we do this to get the artist artwork
          const artist_ids = [];
          for (var i = 0; i < response.artists.length; i++) {
            artist_ids.push(getFromUri('artistid', response.artists[i].uri));
          }

          // get all album artists as full objects
          request(dispatch, getState, `artists/?ids=${artist_ids}`)
            .then(
              (response) => {
                dispatch(coreActions.artistsLoaded(response.artists));
              },
              (error) => {
                dispatch(coreActions.handleException(
                  'Could not load album\'s artists',
                  error,
                ));
              },
            );
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load album',
            error,
          ));
        },
      );
  };
}


/**
 * =============================================================== PLAYLIST(S) ==========
 * ======================================================================================
 * */

export function createPlaylist(name, description, is_public, is_collaborative) {
  return (dispatch, getState) => {
    const data = {
      name,
      description,
      public: is_public,
      collaborative: is_collaborative,
    };

    request(dispatch, getState, `users/${getState().spotify.me.id}/playlists/`, 'POST', data)
      .then(
        (response) => {
          dispatch({
            type: 'PLAYLIST_LOADED',
            key: response.uri,
            playlist: {

              ...response,
              can_edit: true,
              tracks: [],
              tracks_more: null,
              tracks_total: 0,
            },
          });

          dispatch({
            type: 'LIBRARY_PLAYLISTS_LOADED',
            uris: [response.uri],
          });

          dispatch(uiActions.createNotification({ content: 'Created playlist' }));
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not create playlist',
            error,
          ));
        },
      );
  };
}

export function savePlaylist(uri, name, description, is_public, is_collaborative, image) {
  return (dispatch, getState) => {
    const data = {
      name,
      description,
      public: is_public,
      collaborative: is_collaborative,
    };

    // Update the playlist fields
    request(
      dispatch, getState, `users/${getState().spotify.me.id}/playlists/${getFromUri('playlistid', uri)}`, 'PUT', data,
    )
      .then(
        (response) => {
          dispatch(uiActions.createNotification({ level: 'warning', content: 'Playlist saved' }));

          // Save the image
          if (image) {
            request(dispatch, getState, `users/${getState().spotify.me.id}/playlists/${getFromUri('playlistid', uri)}/images`, 'PUT', image)
              .then(

                (response) => {
                  dispatch({
                    type: 'PLAYLIST_UPDATED',
                    key: uri,
                    playlist: {
                      name,
                      public: is_public,
                      collaborative: is_collaborative,
                      description,
                    },
                  });
                },
                (error) => {
                  dispatch(coreActions.handleException(
                    'Could not save image',
                    error,
                  ));
                },
              );

            // No image, so we're done here
          } else {
            dispatch({
              type: 'PLAYLIST_UPDATED',
              key: uri,
              playlist: {
                name,
                public: is_public,
                collaborative: is_collaborative,
                description,
              },
            });
          }
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not save playlist',
            error,
          ));
        },
      );
  };
}

export function getPlaylist(uri) {
  return (dispatch, getState) => {
    // get the main playlist object
    request(dispatch, getState, `playlists/${getFromUri('playlistid', uri)}?market=${getState().spotify.country}`)
      .then(
        (response) => {
          // convert links in description
          let description = null;
          if (response.description) {
            description = response.description;
            description = description.split('<a href="spotify:artist:').join('<a href="#' + '/artist/spotify:artist:');
            description = description.split('<a href="spotify:album:').join('<a href="#' + '/album/spotify:album:');
            description = description.split('<a href="spotify:user:').join('<a href="#' + '/user/spotify:user:');
          }

          const tracks = formatTracks(response.tracks.items);

          const playlist = {

            ...formatPlaylist(response),
            is_completely_loaded: true,
            user_uri: response.owner.uri,
            tracks_uris: tracks ? arrayOf('uri', tracks) : null,
            tracks_more: response.tracks.next,
            tracks_total: response.tracks.total,
            description,
          };

          dispatch(coreActions.userLoaded(formatUser(response.owner)));
          dispatch(coreActions.tracksLoaded(tracks));
          dispatch(coreActions.playlistLoaded(playlist));
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load playlist',
            error,
          ));
        },
      );
  };
}

/**
 * Get all library tracks
 *
 * Recursively get .next until we have all tracks
 * */

export function getLibraryTracksAndPlay(uri) {
  return (dispatch, getState) => {
    dispatch(uiActions.startProcess(
      'SPOTIFY_GET_LIBRARY_TRACKS_AND_PLAY_PROCESSOR',
      'Loading library tracks',
      {
        uri,
        next: 'me/tracks',
      },
    ));
  };
}

export function getLibraryTracksAndPlayProcessor(data) {
  return (dispatch, getState) => {
    request(dispatch, getState, data.next)
      .then(
        (response) => {
          // Check to see if we've been cancelled
          if (getState().ui.processes.SPOTIFY_GET_LIBRARY_TRACKS_AND_PLAY_PROCESSOR !== undefined) {
            const processor = getState().ui.processes.SPOTIFY_GET_LIBRARY_TRACKS_AND_PLAY_PROCESSOR;

            if (processor.status == 'cancelling') {
              dispatch(uiActions.processCancelled('SPOTIFY_GET_LIBRARY_TRACKS_AND_PLAY_PROCESSOR'));
              return false;
            }
          }

          // Add on our new batch of loaded tracks
          let uris = [];
          const new_uris = [];
          for (let i = 0; i < response.items.length; i++) {
            new_uris.push(response.items[i].track.uri);
          }
          if (data.uris) {
            uris = [...data.uris, ...new_uris];
          } else {
            uris = new_uris;
          }

          // We got a next link, so we've got more work to be done
          if (response.next) {
            dispatch(uiActions.updateProcess(
              'SPOTIFY_GET_LIBRARY_TRACKS_AND_PLAY_PROCESSOR',
              `Loading ${response.total - uris.length} library tracks`,
              {
                next: response.next,
                total: response.total,
                remaining: response.total - uris.length,
              },
            ));
            dispatch(uiActions.runProcess(
              'SPOTIFY_GET_LIBRARY_TRACKS_AND_PLAY_PROCESSOR',
              {
                next: response.next,
                uris,
              },
            ));
          } else {
            dispatch(mopidyActions.playURIs(uris, data.uri));
            dispatch(uiActions.processFinished('SPOTIFY_GET_LIBRARY_TRACKS_AND_PLAY_PROCESSOR'));
          }
        },
        () => {
          dispatch(uiActions.processFinished(
            'SPOTIFY_GET_LIBRARY_TRACKS_AND_PLAY_PROCESSOR',
            {
              content: 'Could not load library tracks',
              level: 'error',
            },
          ));
        },
      );
  };
}

/**
 * Get all tracks for a playlist
 *
 * Recursively get .next until we have all tracks
 * */

export function getAllPlaylistTracks(uri, shuffle = false, callback_action = null, play_next = false, at_position = null, offset = 0) {
  return (dispatch, getState) => {
    dispatch(uiActions.startProcess(
      'SPOTIFY_GET_ALL_PLAYLIST_TRACKS_PROCESSOR',
      'Loading playlist tracks',
      {
        uri,
        next: `playlists/${getFromUri('playlistid', uri)}/tracks?market=${getState().spotify.country}`,
        shuffle,
        play_next,
        at_position,
        offset,
        callback_action,
      },
    ));
  };
}

export function getAllPlaylistTracksProcessor(data) {
  return (dispatch, getState) => {
    request(dispatch, getState, data.next)
      .then(
        (response) => {
          // Check to see if we've been cancelled
          if (getState().ui.processes.SPOTIFY_GET_ALL_PLAYLIST_TRACKS_PROCESSOR !== undefined) {
            const processor = getState().ui.processes.SPOTIFY_GET_ALL_PLAYLIST_TRACKS_PROCESSOR;

            if (processor.status == 'cancelling') {
              dispatch(uiActions.processCancelled('SPOTIFY_GET_ALL_PLAYLIST_TRACKS_PROCESSOR'));
              return false;
            }
          }

          // Add on our new batch of loaded tracks
          let tracks = [];
          const new_tracks = [];
          for (const item of response.items) {
            if (item.track) {
              new_tracks.push(item.track);
            }
          }
          if (data.tracks) {
            tracks = [...data.tracks, ...new_tracks];
          } else {
            tracks = new_tracks;
          }

          // We got a next link, so we've got more work to be done
          if (response.next) {
            dispatch(uiActions.updateProcess(
              'SPOTIFY_GET_ALL_PLAYLIST_TRACKS_PROCESSOR',
              `Loading ${response.total - tracks.length} playlist tracks`,
              {
                ...data,
                next: response.next,
                total: response.total,
                remaining: response.total - tracks.length,
              },
            ));
            dispatch(uiActions.runProcess(
              'SPOTIFY_GET_ALL_PLAYLIST_TRACKS_PROCESSOR',
              {
                ...data,
                next: response.next,
                tracks,
              },
            ));
          } else {
            // Seeing as we now have all the playlist's tracks, add them to the playlist we have
            // in our index for quicker reuse next time
            dispatch(coreActions.loadedMore(
              'playlist',
              data.uri,
              'track',
              { tracks },
            ));

            let uris = arrayOf('uri', tracks);

            if (data.shuffle) {
              uris = shuffle(uris);
            }

            // We don't bother "finishing", we just want it "finished" immediately
            // This bypasses the fade transition for a more smooth transition between two
            // processes that flow together
            dispatch(uiActions.removeProcess('SPOTIFY_GET_ALL_PLAYLIST_TRACKS_PROCESSOR'));

            if (data.callback_action == 'enqueue') {
              dispatch(mopidyActions.enqueueURIs(uris, data.uri, data.play_next, data.at_position, data.offset));
            } else {
              dispatch(mopidyActions.playURIs(uris, data.uri));
            }
          }
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not load tracks to play playlist',
            error,
          ));
        },
      );
  };
}

export function addTracksToPlaylist(uri, tracks_uris) {
  return (dispatch, getState) => {
    request(dispatch, getState, `playlists/${getFromUri('playlistid', uri)}/tracks`, 'POST', { uris: tracks_uris })
      .then(
        (response) => {
          dispatch({
            type: 'PLAYLIST_TRACKS_ADDED',
            key: uri,
            tracks_uris,
            snapshot_id: response.snapshot_id,
          });
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not add tracks to playlist',
            error,
          ));
        },
      );
  };
}

export function deleteTracksFromPlaylist(uri, snapshot_id, tracks_indexes) {
  return (dispatch, getState) => {
    request(dispatch, getState, `playlists/${getFromUri('playlistid', uri)}/tracks`, 'DELETE', { snapshot_id, positions: tracks_indexes })
      .then(
        (response) => {
          dispatch({
            type: 'PLAYLIST_TRACKS_REMOVED',
            key: uri,
            tracks_indexes,
            snapshot_id: response.snapshot_id,
          });
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not remove tracks from playlist',
            error,
          ));
        },
      );
  };
}

export function reorderPlaylistTracks(uri, range_start, range_length, insert_before, snapshot_id) {
  return (dispatch, getState) => {
    request(dispatch, getState, `playlists/${getFromUri('playlistid', uri)}/tracks`, 'PUT', {
      uri, range_start, range_length, insert_before, snapshot_id,
    })
      .then(
        (response) => {
          dispatch({
            type: 'PLAYLIST_TRACKS_REORDERED',
            key: uri,
            range_start,
            range_length,
            insert_before,
            snapshot_id: response.snapshot_id,
          });
        },
        (error) => {
          dispatch(coreActions.handleException(
            'Could not reorder playlist tracks',
            error,
          ));
        },
      );
  };
}


/**
 * =============================================================== LIBRARY ==============
 * ======================================================================================
 * */

export function flushLibrary() {
  return {
    type: 'SPOTIFY_FLUSH_LIBRARY',
  };
}


/**
 * Playlists
 * */

export function getLibraryPlaylists() {
  return (dispatch, getState) => {
    const last_run = getState().ui.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR;

    if (!last_run) {
      dispatch(uiActions.startProcess('SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR', 'Loading Spotify playlists', { next: 'me/playlists?limit=50' }));
    } else if (last_run.status === 'cancelled') {
      dispatch(uiActions.resumeProcess('SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR'));

      // We've already finished, but the status has been flushed
    } else if (last_run.status === 'finished' && !getState().spotify.library_playlists_loaded_all) {
      dispatch(uiActions.startProcess('SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR', 'Loading Spotify playlists', { next: 'me/playlists?limit=50' }));
    }
  };
}

export function getLibraryPlaylistsProcessor(data) {
  return (dispatch, getState) => {
    request(dispatch, getState, data.next)
      .then(
        (response) => {
          dispatch({
            type: 'SPOTIFY_LIBRARY_PLAYLISTS_LOADED',
            playlists: response.items,
          });

          // Check to see if we've been cancelled
          if (getState().ui.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR !== undefined) {
            const processor = getState().ui.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR;

            if (processor.status == 'cancelling') {
              dispatch(uiActions.processCancelled('SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR'));
              return false;
            }
          }

          // We got a next link, so we've got more work to be done
          if (response.next) {
            const { total } = response;
            const loaded = getState().spotify.library_playlists.length;
            const remaining = total - loaded;
            dispatch(uiActions.updateProcess(
              'SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR',
              `Loading ${remaining} Spotify playlists`,
              {
                next: response.next,
                total: response.total,
                remaining,
              },
            ));
            dispatch(uiActions.runProcess('SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR', { next: response.next }));
          } else {
            dispatch(uiActions.processFinished('SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR'));
            dispatch({ type: 'SPOTIFY_LIBRARY_PLAYLISTS_LOADED_ALL' });
          }
        },
        () => {
          dispatch(uiActions.processFinished(
            'SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR',
            {
              content: i18n(
                'errors.could_not_load_library',
                {
                  name: i18n('library.playlists.title'),
                  provider: i18n('services.spotify.title'),
                },
              ),
              level: 'error',
            },
          ));
        },
      );
  };
}


/**
 * Artists
 * */

export function getLibraryArtists() {
  return (dispatch, getState) => {
    const last_run = getState().ui.processes.SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR;

    if (!last_run) {
      dispatch(uiActions.startProcess('SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR', 'Loading Spotify artists', { next: 'me/following?type=artist&limit=50' }));
    } else if (last_run.status === 'cancelled') {
      dispatch(uiActions.resumeProcess('SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR'));

      // We've already finished, but the status has been flushed
    } else if (last_run.status === 'finished' && !getState().spotify.library_artists_loaded_all) {
      dispatch(uiActions.startProcess('SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR', 'Loading Spotify artists', { next: 'me/following?type=artist&limit=50' }));
    }
  };
}

export function getLibraryArtistsProcessor(data) {
  return (dispatch, getState) => {
    request(dispatch, getState, data.next)
      .then(
        (response) => {
          dispatch({
            type: 'SPOTIFY_LIBRARY_ARTISTS_LOADED',
            artists: response.artists.items,
          });
          // We got a next link, so we've got more work to be done
          if (response.artists.next) {
            const { total } = response.artists;
            const loaded = getState().spotify.library_artists.length;
            const remaining = total - loaded;
            dispatch(uiActions.updateProcess(
              'SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR',
              `Loading ${remaining} Spotify artists`,
              {
                next: response.artists.next,
                total: response.artists.total,
                remaining,
              },
            ));
            dispatch(uiActions.runProcess('SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR', { next: response.artists.next }));
          } else {
            dispatch(uiActions.processFinished('SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR'));
          }
        },
        () => {
          dispatch(uiActions.processFinished(
            'SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR',
            {
              content: i18n(
                'errors.could_not_load_library',
                {
                  name: i18n('library.artists.title'),
                  provider: i18n('services.spotify.title'),
                },
              ),
              level: 'error',
            },
          ));
        },
      );
  };
}


/**
 * ALbums
 * */

export function getLibraryAlbums() {
  return (dispatch, getState) => {
    const last_run = getState().ui.processes.SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR;

    if (!last_run) {
      dispatch(uiActions.startProcess('SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR', 'Loading Spotify albums', { next: 'me/albums?limit=50' }));
    } else if (last_run.status === 'cancelled') {
      dispatch(uiActions.updateProcess('SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR', 'Loading Spotify albums', { next: 'me/albums?limit=50' }));

      // We've already finished, but the status has been flushed
    } else if (last_run.status === 'finished' && !getState().spotify.library_albums_loaded_all) {
      dispatch(uiActions.startProcess('SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR', 'Loading Spotify albums', { next: 'me/albums?limit=50' }));
    }
  };
}

export function getLibraryAlbumsProcessor(data) {
  return (dispatch, getState) => {
    request(dispatch, getState, data.next)
      .then(
        (response) => {
          dispatch({
            type: 'SPOTIFY_LIBRARY_ALBUMS_LOADED',
            albums: response.items,
          });

          // Check to see if we've been cancelled
          if (getState().ui.processes.SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR !== undefined) {
            const processor = getState().ui.processes.SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR;

            if (processor.status == 'cancelling') {
              dispatch(uiActions.processCancelled('SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR'));
              return false;
            }
          }

          // We got a next link, so we've got more work to be done
          if (response.next) {
            const { total } = response;
            const loaded = getState().spotify.library_albums.length;
            const remaining = total - loaded;
            dispatch(uiActions.updateProcess(
              'SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR',
              `Loading ${remaining} Spotify albums`,
              {
                next: response.next,
                total: response.total,
                remaining,
              },
            ));
            dispatch(uiActions.runProcess('SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR', { next: response.next }));
          } else {
            dispatch(uiActions.processFinished('SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR'));
          }
        },
        () => {
          dispatch(uiActions.processFinished(
            'SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR',
            {
              content: i18n(
                'errors.could_not_load_library',
                {
                  name: i18n('library.albums.title'),
                  provider: i18n('services.spotify.title'),
                },
              ),
              level: 'error',
            },
          ));
        },
      );
  };
}

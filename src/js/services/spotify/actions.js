import React from 'react';
import { arrayOf } from '../../util/arrays';
import {
  generateGuid,
  getFromUri,
  uriType,
  upgradeSpotifyPlaylistUris,
} from '../../util/helpers';
import {
  formatCategory,
  formatCategories,
  formatTracks,
  formatPlaylist,
  formatPlaylists,
  formatUser,
  formatAlbum,
  formatArtist,
  formatArtists,
  formatAlbums,
  formatImages,
  formatTrack,
  injectSortId,
} from '../../util/format';
import URILink from '../../components/URILink';
import { i18n } from '../../locale';
import { getItem, getProvider } from '../../util/selectors';

const coreActions = require('../core/actions');
const uiActions = require('../ui/actions');
const mopidyActions = require('../mopidy/actions');
const lastfmActions = require('../lastfm/actions');
const geniusActions = require('../genius/actions');

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
const request = ({
  dispatch,
  getState,
  endpoint,
  method = 'GET',
  data,
  uri,
}) => {
  // Add reference to loader queue
  // We do this straight away so that even if we're refreshing the token, it still registers as
  // loading said endpoint
  const loaderId = generateGuid();
  const loaderKey = `spotify_${uri ? `uri_${uri}` : ''}_endpoint_${endpoint}`;
  dispatch(uiActions.startLoading(loaderId, loaderKey));

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
            dispatch(uiActions.stopLoading(loaderId));

            // TODO: Rate limiting
            if (response.status === 429) {
              console.error('You hit the Spotify API rate limiter');
            }

            return response.text().then((text) => (text ? JSON.parse(text) : {}));
          }

          fetch(url, config)
            .then(status)
            .then((data) => {
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
            .catch((error) => {
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
        () => {
          // Return myself for a re-check
          getToken(dispatch, getState);
        },
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
    request({ dispatch, getState, endpoint: 'me' })
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

export function getTrack(uri, { forceRefetch, full, lyrics }) {
  return (dispatch, getState) => {
    let endpoint = `tracks/${getFromUri('trackid', uri)}`;
    if (forceRefetch) endpoint += `?refetch=${Date.now()}`;

    request({
      dispatch,
      getState,
      endpoint,
      uri,
    }).then(
      (response) => {
        const track = formatTrack(response);
        dispatch(coreActions.itemLoaded(track));
        if (full) {
          if (getState().lastfm.authorization) {
            dispatch(lastfmActions.getTrack(uri));
          }
        }
        if (lyrics && getState().genius.authorization) {
          dispatch(geniusActions.findTrackLyrics(uri));
        }
      },
    );
  };
}

export function getFeaturedPlaylists(forceRefetch = false) {
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
    let endpoint = 'browse/featured-playlists?limit=50';
    endpoint += `&country=${getState().spotify.country}`;
    endpoint += `&locale=${getState().spotify.locale}`;
    endpoint += `&timestamp=${timestamp}`;
    if (forceRefetch) endpoint += `&refetch=${Date.now()}`;

    request({
      dispatch,
      getState,
      endpoint,
    }).then(
      (response) => {
        const playlists = response.playlists.items.map(
          (raw_playlist) => {
            const playlist = formatPlaylist(raw_playlist);
            delete playlist.tracks; // Don't overwrite (we may already have loaded these)
            return playlist;
          },
        );

        dispatch(coreActions.itemsLoaded(playlists));

        dispatch({
          type: 'SPOTIFY_FEATURED_PLAYLISTS_LOADED',
          data: {
            message: response.message,
            uris: upgradeSpotifyPlaylistUris(arrayOf('uri', playlists)),
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
    let endpoint = 'browse/categories';
    endpoint += '?limit=50';
    endpoint += `&country=${getState().spotify.country}`;
    endpoint += `&locale=${getState().spotify.locale}`;

    request({ dispatch, getState, endpoint })
      .then(
        (response) => {
          dispatch({
            type: 'SPOTIFY_CATEGORIES_LOADED',
            categories: formatCategories(response.categories.items),
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

export function getCategory(uri, { forceRefetch } = {}) {
  return (dispatch, getState) => {
    const loaderId = generateGuid();
    dispatch(uiActions.startLoading(loaderId, `spotify_category_${uri}`));

    const id = getFromUri('categoryid', uri);
    let endpoint = `browse/categories/${id}`;
    endpoint += `?country=${getState().spotify.country}`;
    endpoint += `&locale=${getState().spotify.locale}`;
    if (forceRefetch) endpoint += `&refetch=${Date.now()}`;

    let plEndpoint = `browse/categories/${id}/playlists`;
    plEndpoint += '?limit=50';
    plEndpoint += `&country=${getState().spotify.country}`;
    plEndpoint += `&locale=${getState().spotify.locale}`;
    if (forceRefetch) plEndpoint += `&refetch=${Date.now()}`;

    request({
      dispatch,
      getState,
      endpoint,
      uri,
    }).then(
      (response) => {
        const category = formatCategory(response);

        let playlists = [];
        const fetchPlaylists = (plEndpoint) => request({
          dispatch,
          getState,
          endpoint: plEndpoint,
        }).then((response) => {
          playlists = [...playlists, ...formatPlaylists(response.playlists.items)];
          if (response.playlists.next) {
            fetchPlaylists(response.playlists.next);
          } else {
            dispatch(coreActions.itemLoaded({
              ...category,
              playlists_uris: arrayOf('uri', playlists),
            }));
            dispatch(coreActions.itemsLoaded(playlists));
            dispatch(uiActions.stopLoading(loaderId));
          }
        });
        fetchPlaylists(plEndpoint);
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

export function getNewReleases(forceRefetch = false) {
  return (dispatch, getState) => {
    let endpoint = 'browse/new-releases';
    endpoint += '?limit=50';
    endpoint += '&offset=0';
    endpoint += `&country=${getState().spotify.country}`;
    if (forceRefetch) endpoint += `&refetch=${Date.now()}`;

    request({ dispatch, getState, endpoint })
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

export function getURL(endpoint, action_name, key = false) {
  return (dispatch, getState) => {
    request({ dispatch, getState, endpoint })
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

export function getMore(endpoint, core_action = null, custom_action = null, extra_data = {}) {
  return (dispatch, getState) => {
    request({ dispatch, getState, endpoint })
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

export function getSearchResults({ type, term }, limit = 50, offset = 0) {
  const processKey = 'SPOTIFY_GET_SEARCH_RESULTS';
  return (dispatch, getState) => {
    const {
      spotify: {
        me: {
          id: meId,
        } = {},
      },
    } = getState();
    dispatch(uiActions.startProcess(processKey, { content: 'Searching Spotify' }));

    let typeString = type.replace(/s+$/, '');
    if (typeString === 'all') {
      typeString = 'album,artist,playlist,track';
    }

    let endpoint = `search?q=${term}`;
    endpoint += `&type=${typeString}`;
    endpoint += `&country=${getState().spotify.country}`;
    endpoint += `&limit=${limit}`;
    endpoint += `&offset=${offset}`;

    request({ dispatch, getState, endpoint })
      .then(
        (response) => {
          if (response.tracks !== undefined) {
            dispatch(coreActions.searchResultsLoaded(
              { term, type },
              'tracks',
              formatTracks(response.tracks.items),
            ));
          }

          if (response.artists !== undefined) {
            dispatch(coreActions.searchResultsLoaded(
              { term, type },
              'artists',
              formatArtists(response.artists.items),
            ));
          }

          if (response.albums !== undefined) {
            dispatch(coreActions.searchResultsLoaded(
              { term, type },
              'albums',
              formatAlbums(response.albums.items),
            ));
          }

          if (response.playlists !== undefined) {
            const playlists = response.playlists.items.map((item) => ({
              ...formatPlaylist(item),
              can_edit: (meId === item.owner.id),
            }));
            dispatch(coreActions.searchResultsLoaded(
              { term, type },
              'playlists',
              playlists,
            ));
          }

          dispatch(uiActions.processFinished(processKey));
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

    request({ dispatch, getState, endpoint })
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
            dispatch(coreActions.itemsLoaded(response.artists.items));
          }

          if (response.albums && response.albums.items) {
            dispatch(coreActions.itemsLoaded(response.albums.items));
          }

          if (response.playlists && response.playlists.items) {
            dispatch(coreActions.itemsLoaded(response.playlists.items));
          }

          if (response.tracks && response.tracks.items) {
            dispatch(coreActions.itemsLoaded(response.tracks.items));
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
    const type = uriType(uri);
    let endpoint;
    let data;
    let is_following = null;
    const asset = getItem(getState(), uri) || {};

    if (method === 'PUT') {
      is_following = true;
    } else if (method === 'DELETE') {
      is_following = false;
    }

    switch (type) {
      case 'track':
        if (method === 'GET') {
          endpoint = `me/tracks/contains?ids=${getFromUri('trackid', uri)}`;
        } else {
          endpoint = `me/tracks?ids=${getFromUri('trackid', uri)}`;
        }
        break;
      case 'album':
        if (method === 'GET') {
          endpoint = `me/albums/contains?ids=${getFromUri('albumid', uri)}`;
        } else {
          endpoint = `me/albums?ids=${getFromUri('albumid', uri)}`;
        }
        break;
      case 'artist':
        if (method === 'GET') {
          endpoint = `me/following/contains?type=artist&ids=${getFromUri('artistid', uri)}`;
        } else {
          endpoint = `me/following?type=artist&ids=${getFromUri('artistid', uri)}`;
          data = {};
        }
        break;
      case 'user':
        if (method === 'GET') {
          endpoint = `me/following/contains?type=user&ids=${getFromUri('userid', uri)}`;
        } else {
          endpoint = `me/following?type=user&ids=${getFromUri('userid', uri)}`;
          data = {};
        }
        break;
      case 'playlist':
        const {
          spotify: {
            me: {
              id: meId,
            } = {},
          },
        } = getState();
        if (method === 'GET') {
          endpoint = `playlists/${getFromUri('playlistid', uri)}/followers/contains?ids=${meId}`;
        } else {
          endpoint = `playlists/${getFromUri('playlistid', uri)}/followers?`;
        }
        break;
      default:
        break;
    }

    request({
      dispatch,
      getState,
      endpoint,
      method,
      data,
      uri,
    }).then(
      (response) => {
        if (Array.isArray(response) && response.length > 0) {
          asset.in_library = response[0];
        } else {
          asset.in_library = is_following;
        }

        if (method === 'DELETE') {
          dispatch(coreActions.removeFromLibrary(`spotify:library:${type}s`, uri));
          dispatch(uiActions.createNotification({
            content: <span>
              Removed
              {' '}
              <URILink type={type} uri={uri}>{asset ? asset.name : type}</URILink>
              {' '}
              from library
            </span>,
          }));
        } else if (method === 'PUT' || method === 'POST') {
          dispatch(coreActions.addToLibrary(`spotify:library:${type}s`, asset));
          dispatch(uiActions.createNotification({
            content: <span>
              Added
              {' '}
              <URILink type={type} uri={uri}>{asset ? asset.name : type}</URILink>
              {' '}
              to library
            </span>,
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

      request({ dispatch, getState, endpoint: `artists?ids=${artist_ids}` })
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

      request({ dispatch, getState, endpoint: `tracks?ids=${track_ids}` })
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
 * =============================================================== ARTIST(S) ============
 * ======================================================================================
 * */

/**
 * Get a single artist
 *
 * @param uri string
 * @param full boolean (whether we want a full artist object)
 * */
export function getArtist(uri, { full, forceRefetch } = {}) {
  return (dispatch, getState) => {
    let endpoint = `artists/${getFromUri('artistid', uri)}`;
    if (forceRefetch) endpoint += `?refetch=${Date.now()}`;

    request({
      dispatch,
      getState,
      endpoint,
      uri,
    }).then(
      (response) => {
        const artist = formatArtist(response);
        dispatch(coreActions.itemLoaded(artist));
        dispatch(lastfmActions.getArtist(uri, artist.name, artist.mbid));
      },
    );

    // Do we want a full artist, with all supporting material?
    if (full) {
      // All albums (gets all pages, may take some time to iterate them all)
      let albums = [];
      const fetchAlbums = (endpoint) => request({
        dispatch, getState, endpoint, uri,
      })
        .then((response) => {
          albums = [...albums, ...formatAlbums(response.items)];
          if (response.next) {
            fetchAlbums(`${response.next}${forceRefetch ? `&refetch=${Date.now()}` : ''}`);
          } else {
            dispatch(coreActions.itemLoaded({
              uri,
              albums_uris: arrayOf('uri', albums),
            }));
            dispatch(coreActions.itemsLoaded(albums));
          }
        });
      fetchAlbums(`artists/${getFromUri('artistid', uri)}/albums?limit=50&include_groups=album,single&market=${getState().spotify.country}`);

      // Get top tracks
      let tracksEndpoint = `artists/${getFromUri('artistid', uri)}`;
      tracksEndpoint += `/top-tracks?country=${getState().spotify.country}`;
      if (forceRefetch) tracksEndpoint += `&refetch=${Date.now()}`;
      request({
        dispatch, getState, endpoint: tracksEndpoint, uri,
      })
        .then(
          (response) => {
            dispatch(coreActions.itemLoaded({
              uri,
              tracks: formatTracks(response.tracks),
            }));
          },
        );

      // Related artists
      let relatedEndpoint = `artists/${getFromUri('artistid', uri)}/related-artists`;
      if (forceRefetch) relatedEndpoint += `?refetch=${Date.now()}`;
      request({
        dispatch, getState, endpoint: relatedEndpoint, uri,
      })
        .then(
          (response) => {
            dispatch(coreActions.itemLoaded({
              uri,
              related_artists: formatArtists(response.artists),
            }));
          },
        );
    }
  };
}

// Used to get images for non-Spotify artists
export function getArtistImages(artist) {
  return (dispatch, getState) => {
    request({ dispatch, getState, endpoint: `search?q=${artist.name}&type=artist` })
      .then((response) => {
        if (response.artists.items.length > 0) {
          dispatch(coreActions.itemLoaded({
            uri: artist.uri,
            images: [formatImages(response.artists.items[0].images)],
          }));
        }
      });
  };
}

export function playArtistTopTracks(uri) {
  return (dispatch, getState) => {
    const {
      items: {
        [uri]: artist,
      },
    } = getState().core;

    // Do we have this artist (and their tracks) in our index already?
    if (artist && artist.tracks) {
      const uris = arrayOf('uri', artist.tracks);
      dispatch(mopidyActions.playURIs(uris, uri));
    } else {
      let endpoint = `artists/${getFromUri('artistid', uri)}`;
      endpoint += `/top-tracks?country=${getState().spotify.country}`;
      request({ dispatch, getState, endpoint })
        .then(
          (response) => {
            const uris = arrayOf('uri', response.tracks);
            dispatch(mopidyActions.playURIs(uris, uri));
          },
        );
    }
  };
}

/**
 * =============================================================== USER(S) ==============
 * ======================================================================================
 * */

export function getUser(uri, { full, forceRefetch } = {}) {
  return (dispatch, getState) => {
    const userId = getFromUri('userid', uri);
    let endpoint = `users/${userId}`;
    if (forceRefetch) endpoint += `?refetch=${Date.now()}`;

    request({
      dispatch,
      getState,
      endpoint,
      uri,
    }).then(
      (response) => {
        const user = formatUser(response);
        dispatch(coreActions.itemLoaded(user));
      },
    );

    if (full) {
      let playlists = [];
      const fetchPlaylists = (endpoint) => request({
        dispatch, getState, endpoint, uri,
      })
        .then((response) => {
          playlists = [...playlists, ...formatPlaylists(response.items)];
          if (response.next) {
            fetchPlaylists(`${response.next}${forceRefetch ? `&refetch=${Date.now()}` : ''}`);
          } else {
            dispatch(coreActions.itemLoaded({
              uri,
              playlists_uris: arrayOf('uri', playlists),
            }));
            dispatch(coreActions.itemsLoaded(playlists));
          }
        });
      fetchPlaylists(
        `users/${userId}/playlists?limit=40${forceRefetch ? `&refetch=${Date.now()}` : ''}`,
      );
    }
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
export function getAlbum(uri, { full, forceRefetch } = {}) {
  return (dispatch, getState) => {
    let endpoint = `albums/${getFromUri('albumid', uri)}`;
    if (forceRefetch) endpoint += `?refetch=${Date.now()}`;

    request({
      dispatch,
      getState,
      endpoint,
      uri,
    }).then(
      (response) => {
        dispatch(coreActions.itemLoaded({
          ...formatAlbum(response),
        }));

        if (full) {
          let tracks = formatTracks(response.tracks.items);
          const fetchTracks = (endpoint) => request({
            dispatch,
            getState,
            endpoint,
            uri,
          }).then(
            (response) => {
              tracks = [...tracks, ...formatTracks(response.items)];
              if (response.next) {
                fetchTracks(`${response.next}${forceRefetch ? `&refetch=${Date.now()}` : ''}`);
              } else {
                dispatch(coreActions.itemLoaded({
                  uri,
                  tracks,
                }));
              }
            },
          );

          if (response.tracks.next) {
            fetchTracks(response.tracks.next);
          }
        }
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

export function createPlaylist(playlist) {
  return (dispatch, getState) => {
    const data = {
      name: playlist.name,
      description: playlist.description || '',
      public: playlist.public,
      collaborative: playlist.collaborative,
    };
    const {
      spotify: {
        me: {
          id: meId,
        } = {},
      },
    } = getState();

    request({
      dispatch,
      getState,
      endpoint: `users/${meId}/playlists/`,
      method: 'POST',
      data,
    })
      .then(
        (response) => {
          dispatch(coreActions.itemLoaded({
            ...formatPlaylist(response),
            can_edit: true,
            tracks: [],
          }));

          dispatch(coreActions.addToLibrary(
            getProvider('playlists', 'spotify:')?.uri,
            response,
          ));

          dispatch(uiActions.createNotification({
            content: i18n('actions.created', { name: i18n('playlist.title') }),
          }));
          if (playlist.tracks_uris) {
            dispatch(coreActions.addTracksToPlaylist(response.uri, playlist.tracks_uris));
          }
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
    const {
      spotify: {
        me: {
          id: meId,
        } = {},
      },
    } = getState();

    // Update the playlist fields
    request({
      dispatch,
      getState,
      endpoint: `users/${meId}/playlists/${getFromUri('playlistid', uri)}`,
      method: 'PUT',
      data,
      uri,
    })
      .then(
        () => {
          dispatch(uiActions.createNotification({ level: 'warning', content: 'Playlist saved' }));

          // Save the image
          if (image) {
            request({
              dispatch,
              getState,
              endpoint: `users/${meId}/playlists/${getFromUri('playlistid', uri)}/images`,
              method: 'PUT',
              data: image,
            }).then(
              () => {
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

export function getPlaylistTracks(uri, { forceRefetch, callbackAction } = {}) {
  return (dispatch, getState) => {
    let initialEndpoint = `playlists/${getFromUri('playlistid', uri)}/tracks`;
    initialEndpoint += `?market=${getState().spotify.country}`;
    if (forceRefetch) initialEndpoint += `&refetch=${Date.now()}`;

    let tracks = [];

    const fetchTracks = (endpoint) => request({
      dispatch, getState, endpoint, uri,
    })
      .then((response) => {
        tracks = [...tracks, ...formatTracks(response.items)];
        if (response.next) {
          fetchTracks(`${response.next}${forceRefetch ? `&refetch=${Date.now()}` : ''}`);
        } else {
          dispatch(coreActions.itemLoaded({
            uri,
            tracks: injectSortId(tracks), // only inject sort_id when we've loaded them all
          }));

          if (callbackAction) {
            switch (callbackAction.name) {
              case 'enqueue':
                dispatch(mopidyActions.enqueueURIs(
                  arrayOf('uri', tracks),
                  uri,
                  callbackAction.play_next,
                  callbackAction.at_position,
                  callbackAction.offset,
                ));
                break;
              case 'play':
                dispatch(mopidyActions.playURIs(
                  arrayOf('uri', tracks),
                  uri,
                  callbackAction.shuffle,
                ));
                break;
              default:
                break;
            }
          }
        }
      });

    fetchTracks(initialEndpoint);
  };
}

export function getPlaylist(uri, options = {}) {
  const { full, forceRefetch } = options;

  return (dispatch, getState) => {
    const {
      spotify: {
        me: {
          id: meId,
        } = {},
      },
    } = getState();
    let endpoint = `playlists/${getFromUri('playlistid', uri)}`;
    endpoint += `?market=${getState().spotify.country}`;
    if (forceRefetch) endpoint += `&refetch=${Date.now()}`;

    request({
      dispatch, getState, endpoint, uri,
    })
      .then(
        (response) => {
          let description = null;
          if (response.description) {
            description = response.description;
            description = description.split('<a href="spotify:artist:').join('<a href="#' + '/artist/spotify:artist:');
            description = description.split('<a href="spotify:album:').join('<a href="#' + '/album/spotify:album:');
            description = description.split('<a href="spotify:user:').join('<a href="#' + '/user/spotify:user:');
          }

          dispatch(coreActions.itemLoaded({
            ...formatPlaylist(response),
            can_edit: (meId === response.owner.id),
            description,
            // Remove tracks. They're handed by another query which allows our detector
            // to accurately identify whether we've loaded *ALL* the tracks. Without this, it
            // doesn't know if we've loaded all tracks, or just the first page.
            tracks: null,
          }));

          if (full) {
            dispatch(getPlaylistTracks(uri, options));
          }
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

export function addTracksToPlaylist(uri, tracks_uris) {
  return (dispatch, getState) => {
    request({
      dispatch,
      getState,
      endpoint: `playlists/${getFromUri('playlistid', uri)}/tracks`,
      method: 'POST',
      data: { uris: tracks_uris },
      uri,
    }).then(
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
    request({
      dispatch,
      getState,
      endpoint: `playlists/${getFromUri('playlistid', uri)}/tracks`,
      method: 'DELETE',
      data: { snapshot_id, positions: tracks_indexes },
      uri,
    }).then(
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
    request({
      dispatch,
      getState,
      endpoint: `playlists/${getFromUri('playlistid', uri)}/tracks`,
      method: 'PUT',
      data: {
        uri, range_start, range_length, insert_before, snapshot_id,
      },
      uri,
    }).then(
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
 * =============================================================== DISCOVER =============
 * ======================================================================================
 * */

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
        default:
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

    request({ dispatch, getState, endpoint })
      .then(
        (response) => {
          const tracks = Object.assign([], formatTracks(response.tracks));

          // We only get simple artist objects, so we need to
          // get the full object. We'll add URIs to our recommendations
          // anyway so we can proceed in the meantime
          const artists_uris = [];
          if (tracks.length > artists_ids.length && tracks.length > 10) {
            while (artists_uris.length < 6) {
              const randomArtistIndex = Math.round(Math.random() * (tracks.length - 1));
              const artist = tracks[randomArtistIndex].artists[0];

              // Make sure this artist is not already in our sample, and
              // is not one of the seeds
              if (!artists_uris.includes(artist.uri) && !artists_ids.includes(artist.id)) {
                artists_uris.push(artist.uri);
                dispatch(getArtist(artist.uri));
              }
            }
          }

          // Copy already loaded albums into array
          const albums_uris = [];
          if (tracks.length > 10) {
            while (albums_uris.length < 6) {
              const randomAlbumIndex = Math.round(Math.random() * (tracks.length - 1));
              const { album } = tracks[randomAlbumIndex];

              // Make sure this album is not already in our sample
              if (!albums_uris.includes(album.uri)) {
                albums_uris.push(album.uri);
                dispatch(getAlbum(album.uri));
              }
            }
          }

          if (tracks.length > 0) {
            dispatch(coreActions.itemsLoaded(tracks));
          }

          dispatch({
            type: 'SPOTIFY_RECOMMENDATIONS_LOADED',
            seeds_uris: uris,
            tracks_uris: arrayOf('uri', tracks),
            artists_uris,
            albums_uris,
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
    request({ dispatch, getState, endpoint: 'recommendations/available-genre-seeds' })
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
 * =============================================================== LIBRARY ==============
 * ======================================================================================
 * */

export function flushLibrary() {
  return {
    type: 'SPOTIFY_FLUSH_LIBRARY',
  };
}

export function getLibraryPlaylists(forceRefetch) {
  return (dispatch, getState) => {
    const {
      spotify: {
        me: {
          id: meId,
        } = {},
      },
    } = getState();
    const processKey = 'SPOTIFY_GET_LIBRARY_PLAYLISTS';
    dispatch(uiActions.startProcess(processKey, { notification: false }));

    let libraryItems = [];
    const fetch = (endpoint) => request({ dispatch, getState, endpoint })
      .then((response) => {
        const processor = getState().ui.processes[processKey];
        if (processor && processor.status === 'cancelling') {
          dispatch(uiActions.stopLoading('spotify:library:playlists'));
          dispatch(uiActions.processCancelled(processKey));
          return;
        }

        dispatch(uiActions.updateProcess(
          processKey,
          { total: response.total, remaining: response.total - libraryItems.length },
        ));

        const items = response.items.map(
          (item) => ({
            ...formatPlaylist(item),
            in_library: true,
            can_edit: (meId === item.owner.id),
          }),
        );
        libraryItems = [...libraryItems, ...items];
        if (response.next) {
          fetch(`${response.next}${forceRefetch ? `&refetch=${Date.now()}` : ''}`);
        } else {
          dispatch(uiActions.processFinished(processKey));
          dispatch(coreActions.itemsLoaded(libraryItems));
          dispatch(coreActions.libraryLoaded({
            uri: 'spotify:library:playlists',
            type: 'playlists',
            items_uris: arrayOf('uri', libraryItems),
          }));
        }
      });

    fetch(`me/playlists?limit=50${forceRefetch ? `&refetch=${Date.now()}` : ''}`);
  };
}

export function getLibraryAlbums(forceRefetch) {
  return (dispatch, getState) => {
    const processKey = 'SPOTIFY_GET_LIBRARY_ALBUMS';
    dispatch(uiActions.startProcess(processKey, { notification: false }));

    let libraryItems = [];
    const fetch = (endpoint) => request({ dispatch, getState, endpoint })
      .then((response) => {
        const processor = getState().ui.processes[processKey];
        if (processor && processor.status === 'cancelling') {
          dispatch(uiActions.processCancelled(processKey));
          dispatch(uiActions.stopLoading('spotify:library:albums'));
          return;
        }

        dispatch(uiActions.updateProcess(
          processKey,
          { total: response.total, remaining: response.total - libraryItems.length },
        ));

        const items = response.items.map(
          (item) => ({
            ...formatAlbum(item),
            in_library: true,
          }),
        );
        libraryItems = [...libraryItems, ...items];

        if (response.next) {
          fetch(`${response.next}${forceRefetch ? `&refetch=${Date.now()}` : ''}`);
        } else {
          dispatch(uiActions.processFinished(processKey));
          dispatch(coreActions.itemsLoaded(libraryItems));
          dispatch(coreActions.libraryLoaded({
            uri: 'spotify:library:albums',
            type: 'albums',
            items_uris: arrayOf('uri', libraryItems),
          }));
        }
      });

    fetch(`me/albums?limit=50${forceRefetch ? `&refetch=${Date.now()}` : ''}`);
  };
}

export function getLibraryArtists(forceRefetch) {
  return (dispatch, getState) => {
    const processKey = 'SPOTIFY_GET_LIBRARY_ARTISTS';
    dispatch(uiActions.startProcess(processKey, { notification: false }));

    let libraryItems = [];
    const fetch = (endpoint) => request({ dispatch, getState, endpoint })
      .then((response) => {
        const processor = getState().ui.processes[processKey];
        if (processor && processor.status === 'cancelling') {
          dispatch(uiActions.processCancelled(processKey));
          dispatch(uiActions.stopLoading('spotify:library:artists'));
          return;
        }

        dispatch(uiActions.updateProcess(
          processKey,
          {
            total: response.artists.total,
            remaining: response.artists.total - libraryItems.length,
          },
        ));

        const items = response.artists.items.map(
          (item) => ({
            ...formatArtist(item),
            in_library: true,
          }),
        );
        libraryItems = [...libraryItems, ...items];
        if (response.artists.next) {
          fetch(`${response.artists.next}${forceRefetch ? `&refetch=${Date.now()}` : ''}`);
        } else {
          dispatch(uiActions.processFinished(processKey));
          dispatch(coreActions.itemsLoaded(libraryItems));
          dispatch(coreActions.libraryLoaded({
            uri: 'spotify:library:artists',
            type: 'artists',
            items_uris: arrayOf('uri', libraryItems),
          }));
        }
      });

    fetch(`me/following?type=artist&limit=50${forceRefetch ? `&refetch=${Date.now()}` : ''}`);
  };
}

export function getLibraryTracks(forceRefetch) {
  return (dispatch, getState) => {
    const processKey = 'SPOTIFY_GET_LIBRARY_TRACKS';
    dispatch(uiActions.startProcess(processKey, { notification: false }));

    let libraryItems = [];
    const fetch = (endpoint) => request({ dispatch, getState, endpoint })
      .then((response) => {
        const processor = getState().ui.processes[processKey];
        if (processor && processor.status === 'cancelling') {
          dispatch(uiActions.processCancelled(processKey));
          dispatch(uiActions.stopLoading('spotify:library:tracks'));
          return;
        }

        dispatch(uiActions.updateProcess(
          processKey,
          { total: response.total, remaining: response.total - libraryItems.length },
        ));

        const items = response.items.map(
          (item) => ({
            ...formatTrack(item),
            in_library: true,
          }),
        );
        libraryItems = [...libraryItems, ...items];
        if (response.next) {
          fetch(`${response.next}${forceRefetch ? `&refetch=${Date.now()}` : ''}`);
        } else {
          dispatch(uiActions.processFinished(processKey));
          dispatch(coreActions.itemsLoaded(libraryItems));
          dispatch(coreActions.libraryLoaded({
            uri: 'spotify:library:tracks',
            type: 'tracks',
            items_uris: arrayOf('uri', libraryItems),
          }));
        }
      });

    fetch(`me/tracks?limit=50${forceRefetch ? `&refetch=${Date.now()}` : ''}`);
  };
}

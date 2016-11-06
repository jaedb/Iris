
var helpers = require('../../helpers.js')

/**
 * Send an ajax request to the Spotify API
 *
 * @param dispatch obj
 * @param getState obj
 * @param endpoint string = the url to query (ie /albums/:uri)
 * @param method string
 * @param data mixed = request payload
 * @return Promise
 **/
const sendRequest = ( dispatch, getState, endpoint, method = 'GET', data = false) => {
    return new Promise( (resolve, reject) => {         
        getToken( dispatch, getState )
            .then( response => {

                var url = 'https://api.spotify.com/v1/'+endpoint
                if( endpoint.startsWith('https://api.spotify.com/') ) url = endpoint;

                $.ajax({
                        method: method,
                        cache: true,
                        url: url,
                        headers: {
                            Authorization: 'Bearer '+ response
                        },
                        data: data
                    }).then( 
                        response => resolve(response),
                        (xhr, status, error) => {
                            console.error( endpoint+' failed', xhr.responseText)
                            reject(error)
                        }
                    )
            });
        }
    );
}


/**
* Check an access token validity
*
* @return Promise
**/
function getToken( dispatch, getState ){
    return new Promise( (resolve, reject) => {

        // token is okay for now, so just resolve with the current token
        if( new Date().getTime() < getState().spotify.token_expiry ){
            resolve(getState().spotify.access_token);
            return
        }

        // token is expiring/expired, so go get a new one and resolve that
        refreshToken( dispatch, getState )
            .then(
                response => resolve(response.access_token),
                error => {
                    dispatch({ type: 'SPOTIFY_DISCONNECTED' })
                    reject(error)
                }
            );
    });
}

function refreshToken( dispatch, getState ){
    return new Promise( (resolve, reject) => {

        if( getState().spotify.authorized ){

            $.ajax({
                    method: 'GET',
                    url: '//jamesbarnsley.co.nz/auth.php?action=refresh&refresh_token='+getState().spotify.refresh_token,
                    dataType: "json",
                    timeout: 10000
                })
                .then(
                    response => {
                        response.token_expiry = new Date().getTime() + ( response.expires_in * 1000 );
                        response.source = 'spotify';
                        dispatch({
                            type: 'SPOTIFY_TOKEN_REFRESHED',
                            data: response
                        });
                        resolve(response);
                    },
                    error => {
                        console.error('Could not refresh token', error)
                        reject(error)
                    }
                );

        }else{

            $.ajax({
                    method: 'GET',
                    url: '//'+getState().mopidy.host+':'+getState().mopidy.port+'/iris/http/refresh_spotify_token',
                    dataType: "json",
                    timeout: 10000
                })
                .then(
                    response => {
                        response.token_expiry = new Date().getTime() + ( response.expires_in * 1000 );
                        response.source = 'mopidy';
                        dispatch({
                            type: 'SPOTIFY_TOKEN_REFRESHED',
                            data: response
                        });
                        resolve(response);
                    },
                    error => {
                        console.error('Could not refresh token', error)
                        reject(error)
                    }
                );
        }

    })
}


/**
 * Actions and Action Creators
 **/

export function setConfig( config ){
    return {
        type: 'SPOTIFY_SET_CONFIG',
        config: config
    }
}

export function connect(){
    return (dispatch, getState) => {

        dispatch({ type: 'SPOTIFY_CONNECTING' });

        sendRequest( dispatch, getState, 'browse/categories?limit=1' )
            .then( response => {
                dispatch({
                    type: 'SPOTIFY_CONNECTED'
                });
            });
    }
}


/**
 * Handle authorization process
 **/

export function startAuthorization(){
    return { type: 'SPOTIFY_START_AUTHORIZATION' }
}

export function authorizationGranted( data ){
    data.token_expiry = new Date().getTime() + data.expires_in;
    return { type: 'SPOTIFY_AUTHORIZATION_GRANTED', data: data }
}

export function authorizationRevoked(){
    return { type: 'SPOTIFY_AUTHORIZATION_REVOKED' }
}

export function refreshingToken(){
    return (dispatch, getState) => {
        dispatch({ type: 'SPOTIFY_TOKEN_REFRESHING' });
        refreshToken( dispatch, getState );
    }
}


/**
 * Get current user
 **/
export function getMe(){
    return (dispatch, getState) => {

        // flush out the previous store value
        dispatch({ type: 'SPOTIFY_ME_LOADED', data: false });

        sendRequest( dispatch, getState, 'me' )
            .then( response => {
                dispatch({
                    type: 'SPOTIFY_ME_LOADED',
                    data: response
                });
            });
    }
}


/**
 * Get a single track
 *
 * @param uri string
 **/
export function getTrack( uri ){
    return (dispatch, getState) => {

        // flush out the previous store value
        dispatch({ type: 'SPOTIFY_TRACK_LOADED', data: false });

        sendRequest( dispatch, getState, 'tracks/'+ helpers.getFromUri('trackid', uri) )
            .then( response => {
                    dispatch({
                        type: 'SPOTIFY_TRACK_LOADED',
                        data: response
                    });
                }
            );
    }
}


/**
 * Get a single artist
 *
 * @param uri string
 **/
export function getArtist( uri ){
    return (dispatch, getState) => {

        // flush out the previous store value
        dispatch({ type: 'SPOTIFY_ARTIST_LOADED', data: false });

        var artist = {};

        // while we're fiddling about, go get the albums
        dispatch(getArtistAlbums(uri));

        // get both the artist and the top tracks
        $.when(

            sendRequest( dispatch, getState, 'artists/'+ helpers.getFromUri('artistid', uri) )
                .then( response => {
                    Object.assign(artist, response);
                }),

            sendRequest( dispatch, getState, 'artists/'+ helpers.getFromUri('artistid', uri) +'/top-tracks?country='+getState().spotify.country )
                .then( response => {
                    Object.assign(artist, response);
                }),

            sendRequest( dispatch, getState, 'artists/'+ helpers.getFromUri('artistid', uri) +'/related-artists' )
                .then( response => {
                    Object.assign(artist, { related_artists: response.artists });
                })

        ).then( () => {
            dispatch({
                type: 'SPOTIFY_ARTIST_LOADED',
                data: artist
            });
        });
    }
}

export function getArtists( uris ){
	return (dispatch, getState) => {

		// flush out the previous store value
        dispatch({ type: 'SPOTIFY_ARTISTS_LOADED', data: false });

        // now get all the artists for this album (full objects)
        var ids = '';
        for( var i = 0; i < uris.length; i++ ){
            if( ids != '' ) ids += ','
            ids += helpers.getFromUri( 'artistid', uris[i] );
        }

        sendRequest( dispatch, getState, 'artists/?ids='+ids )
            .then( response => {
                dispatch({
                    type: 'SPOTIFY_ARTISTS_LOADED',
                    data: response.artists
                });
            });
	}
}

export function getArtistAlbums( uri ){
    return (dispatch, getState) => {
        sendRequest( dispatch, getState, 'artists/'+ helpers.getFromUri('artistid', uri) +'/albums' )
            .then( response => {
                dispatch({
                	type: 'SPOTIFY_ARTIST_ALBUMS_LOADED',
                	data: response
                });
            });
	}
}

/**
 * Single album
 *
 * @oaram uri string
 **/
export function getAlbum( uri ){
    return (dispatch, getState) => {

        // flush out the previous store value
        dispatch({ type: 'SPOTIFY_ALBUM_LOADED', data: false });

        sendRequest( dispatch, getState, 'albums/'+ helpers.getFromUri('albumid', uri) )
            .then( response => {

                // now get all the artists for this album (full objects)
                var artist_uris = [];
                for( var i = 0; i < response.artists.length; i++ ){
                    artist_uris.push( response.artists[i].uri )
                }
                dispatch(getArtists(artist_uris));

                // inject the parent album object into each track for consistent track objects
                for( var i = 0; i < response.tracks.items.length; i++ ){
                    response.tracks.items[i].album = {
                        name: response.name,
                        uri: response.uri
                    }
                }

                dispatch({
                    type: 'SPOTIFY_ALBUM_LOADED',
                    data: response
                });
            });
    }
}

/**
 * Single playlist
 *
 * @oaram uri string
 **/
export function getPlaylist( uri ){
    return (dispatch, getState) => {

		// flush out the previous store value
        dispatch({ type: 'SPOTIFY_PLAYLIST_LOADED', data: false });

        sendRequest( dispatch, getState, 'users/'+ helpers.getFromUri('userid',uri) +'/playlists/'+ helpers.getFromUri('playlistid',uri) +'?market='+getState().spotify.country )
            .then( response => {
                dispatch({
                	type: 'SPOTIFY_PLAYLIST_LOADED',
                	data: response
                });
            });
	}
}


export function getLibraryPlaylists(){
    return (dispatch, getState) => {

        dispatch({ type: 'SPOTIFY_LIBRARY_PLAYLISTS_LOADED', data: false });

        sendRequest( dispatch, getState, 'me/playlists' )
            .then( response => {
                dispatch({
                    type: 'SPOTIFY_LIBRARY_PLAYLISTS_LOADED',
                    data: response
                });
            });
    }
}


export function getLibraryArtists(){
	return (dispatch, getState) => {

        dispatch({ type: 'SPOTIFY_LIBRARY_ARTISTS_LOADED', data: false });

        sendRequest( dispatch, getState, 'me/following?type=artist' )
            .then( response => {
                dispatch({
                	type: 'SPOTIFY_LIBRARY_ARTISTS_LOADED',
                	data: response.artists
                });
            });
	}
}


export function getLibraryAlbums(){
	return (dispatch, getState) => {

        dispatch({ type: 'SPOTIFY_LIBRARY_ALBUMS_LOADED', data: false });

        sendRequest( dispatch, getState, 'me/albums' )
            .then( response => {
                dispatch({
                	type: 'SPOTIFY_LIBRARY_ALBUMS_LOADED',
                	data: response
                });
            });
	}
}

export function getLibraryTracks(){
    return (dispatch, getState) => {

        dispatch({ type: 'SPOTIFY_LIBRARY_TRACKS_LOADED', data: false });

        sendRequest( dispatch, getState, 'me/tracks?limit=50' )
            .then( response => {
                dispatch({
                    type: 'SPOTIFY_LIBRARY_TRACKS_LOADED',
                    data: response
                });
            });
    }
}

export function getFeaturedPlaylists(){
    return (dispatch, getState) => {

        dispatch({ type: 'SPOTIFY_FEATURED_PLAYLISTS_LOADED', data: false });

        var date = new Date();
        var year = date.getFullYear();
        var month = date.getMonth();
        if( month < 10 ) month = '0'+month;
        var day = date.getDay();
        if( day < 10 ) day = '0'+day;
        var hour = date.getHours();
        if( hour < 10 ) hour = '0'+hour;
        var min = date.getMinutes();
        if( min < 10 ) min = '0'+min;
        var sec = date.getSeconds();
        if( sec < 10 ) sec = '0'+sec;

        var timestamp = year+'-'+month+'-'+day+'T'+hour+':'+min+':'+sec;

        sendRequest( dispatch, getState, 'browse/featured-playlists?timestamp='+timestamp+'&country='+getState().spotify.country+'&limit=50' )
            .then( response => {
                dispatch({
                    type: 'SPOTIFY_FEATURED_PLAYLISTS_LOADED',
                    data: response
                });
            });
    }
}

export function getCategories(){
    return (dispatch, getState) => {

        dispatch({ type: 'SPOTIFY_CATEGORIES_LOADED', data: false });

        sendRequest( dispatch, getState, 'browse/categories?limit=50' )
            .then( response => {
                dispatch({
                    type: 'SPOTIFY_CATEGORIES_LOADED',
                    data: response.categories
                });
            });
    }
}

export function getCategory( id ){
    return (dispatch, getState) => {

        dispatch({ type: 'SPOTIFY_CATEGORY_LOADED', data: false });

        sendRequest( dispatch, getState, 'browse/categories/'+id )
            .then( response => {
                dispatch({
                    type: 'SPOTIFY_CATEGORY_LOADED',
                    data: response
                });
            });
    }
}

export function getCategoryPlaylists( id ){
    return (dispatch, getState) => {

        dispatch({ type: 'SPOTIFY_CATEGORY_PLAYLISTS_LOADED', data: false });

        sendRequest( dispatch, getState, 'browse/categories/'+id+'/playlists?limit=50' )
            .then( response => {
                dispatch({
                    type: 'SPOTIFY_CATEGORY_PLAYLISTS_LOADED',
                    data: response.playlists
                });
            });
    }
}

export function getNewReleases(){
    return (dispatch, getState) => {

        dispatch({ type: 'SPOTIFY_NEW_RELEASES_LOADED', data: false });

        sendRequest( dispatch, getState, 'browse/new-releases?country='+getState().spotify.country+'&limit=50' )
            .then( response => {
                dispatch({
                    type: 'SPOTIFY_NEW_RELEASES_LOADED',
                    data: response.albums
                });
            });
    }
}

export function getURL( url, action_name ){
    return (dispatch, getState) => {
        sendRequest( dispatch, getState, url )
            .then( response => {
                dispatch({
                    type: action_name,
                    data: response
                });
            });
    }
}

export function getSearchResults( query, type = 'album,artist,playlist,track', limit = 50, offset = 0 ){
	return (dispatch, getState) => {

        dispatch({ type: 'SPOTIFY_SEARCH_RESULTS_LOADED', data: false });

        var url = 'search?q='+query
        url += '&type='+type
        url += '&country='+getState().spotify.country
        url += '&limit='+limit
        url += '&offset='+offset

        sendRequest( dispatch, getState, url )
            .then( response => {
                dispatch({
                    type: 'SPOTIFY_SEARCH_RESULTS_LOADED',
                    data: response
                });
            });
	}
}

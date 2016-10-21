
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
	return {
		type: 'SPOTIFY_CONNECTING'
	}
}

export function disconnect(){
	return {
		type: 'SPOTIFY_DISCONNECT'
	}
}

export function loadAlbum( uri ){
	return {
		type: 'SPOTIFY_LOAD_ALBUM',
		uri: uri
	}
}


/**
 * Send an ajax request to the Spotify API
 *
 * @param endpoint string = the url to query (ie /albums/:uri)
 * @param method string
 * @param data mixed = request payload
 * @return ajax promise
 **/
const sendRequest = (token, endpoint, method = 'GET', data = false) => {

    var options = {
        method: method,
        cache: true,
        url: 'https://api.spotify.com/v1/'+endpoint,
        data: data
    };

    if( token ){
        options.headers = {
            Authorization: 'Bearer '+ token
        }
    }

    return $.ajax( options );
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

export function removeAuthorization(){
	return { type: 'SPOTIFY_REMOVE_AUTHORIZATION' }
}


/**
* Check an access token validity
*
* @return Promise
**/
function checkToken( dispatch, getState ){
    return new Promise( (resolve, reject) => {

        // is our token_expiry still in the future?
        if( new Date().getTime() < getState().spotify.token_expiry ){
            resolve();
            return
        }
        
        // token is expiring/expired
        doRefreshToken( dispatch, getState )
            .then(
                response => {
                    resolve();
                },
                error => reject(error)
            );
    });
}

function doRefreshToken( dispatch, getState ){
    return new Promise( (resolve, reject) => {
        $.ajax({
                method: 'GET',
                url: '//jamesbarnsley.co.nz/spotmop.php?action=refresh&refresh_token='+getState().spotify.refresh_token,
                dataType: "json",
                timeout: 10000
            })
            .then(
                response => {
                    response.token_expiry = new Date().getTime() + response.expires_in;
                    dispatch({
                        type: 'SPOTIFY_TOKEN_REFRESHED',
                        data: response
                    });
                    resolve();
                },
                error => reject(error)
            );
    })
}

export function refreshToken(){
    return (dispatch, getState) => {
        dispatch({ type: 'SPOTIFY_TOKEN_REFRESHING' });
        doRefreshToken( dispatch, getState );
    }
}


/**
 * Get current user
 **/
export function getMe(){
    return (dispatch, getState) => {

        // flush out the previous store value
        dispatch({ type: 'SPOTIFY_ME_LOADED', data: false });

        sendRequest( getState().spotify.access_token, 'me' )
            .then( response => {
                dispatch({
                    type: 'SPOTIFY_ME_LOADED',
                    data: response
                });
            });
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

	        sendRequest( false, 'artists/'+ getFromUri('artistid', uri) )
	            .then( response => {
	            	Object.assign(artist, response);
	            }),

	        sendRequest( false, 'artists/'+ getFromUri('artistid', uri) +'/top-tracks?country='+getState().spotify.country )
	            .then( response => {
	            	Object.assign(artist, response);
	            })

	    ).then( () => {
            dispatch({
            	type: 'SPOTIFY_ARTIST_LOADED',
            	data: artist
            });
	    });
	}
}

export function getArtistAlbums( uri ){
	return dispatch => {
        sendRequest( false, 'artists/'+ getFromUri('artistid', uri) +'/albums' )
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
    return dispatch => {

        // flush out the previous store value
        dispatch({ type: 'SPOTIFY_ALBUM_LOADED', data: false });

        sendRequest( false, 'albums/'+ getFromUri('albumid', uri) )
            .then( response => {

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

        sendRequest( getState().spotify.access_token, 'users/'+ getFromUri('userid',uri) +'/playlists/'+ getFromUri('playlistid',uri) +'?market='+getState().spotify.country )
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

        sendRequest( getState().spotify.access_token, 'me/playlists' )
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

        sendRequest( getState().spotify.access_token, 'me/following?type=artist' )
            .then( response => {
                dispatch({
                	type: 'SPOTIFY_LIBRARY_ARTISTS_LOADED',
                	data: response
                });
            });
	}
}


export function getLibraryAlbums(){
	return (dispatch, getState) => {

        dispatch({ type: 'SPOTIFY_LIBRARY_ALBUMS_LOADED', data: false });

        sendRequest( getState().spotify.access_token, 'me/albums' )
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

        checkToken( dispatch, getState )
            .then( () => {

                dispatch({ type: 'SPOTIFY_LIBRARY_TRACKS_LOADED', data: false });

                sendRequest( getState().spotify.access_token, 'me/tracks?limit=50' )
                    .then( response => {
                        dispatch({
                            type: 'SPOTIFY_LIBRARY_TRACKS_LOADED',
                            data: response
                        });
                    });
            });
	}
}





/**
 * Get an element from a URI
 * @param element = string, the element we wish to extract
 * @param uri = string
 **/
function getFromUri( element, uri ){
    var exploded = uri.split(':');          
    if( element == 'userid' && exploded[1] == 'user' )
        return exploded[2];             
    if( element == 'playlistid' && exploded[3] == 'playlist' )
        return exploded[4];
    if( element == 'artistid' && exploded[1] == 'artist' )
        return exploded[2];             
    if( element == 'albumid' && exploded[1] == 'album' )
        return exploded[2];             
    if( element == 'trackid' && exploded[1] == 'track' )
        return exploded[2];             
    return null;
}

/**
 * Identify what kind of asset a URI is (playlist, album, etc)
 * @param uri = string
 * @return string
 **/
function uriType( uri ){
    var exploded = uri.split(':');
    if( exploded[0] == 'spotify' && exploded[1] == 'track' )
        return 'track'; 
    if( exploded[0] == 'spotify' && exploded[1] == 'artist' )
        return 'artist';        
    if( exploded[0] == 'spotify' && exploded[1] == 'album' )
        return 'album';     
    if( exploded[0] == 'spotify' && exploded[1] == 'user' && exploded[3] == 'playlist' )
        return 'playlist';      
    if( exploded[0] == 'spotify' && exploded[1] == 'user' && exploded.length == 3 )
        return 'user';      
    return null;
}

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
	return { type: 'SPOTIFY_AUTHORIZATION_GRANTED', data: data }
}

export function removeAuthorization(){
	return { type: 'SPOTIFY_REMOVE_AUTHORIZATION' }
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

		var id = uri.replace('spotify:artist:','');
		var artist = {};

		// while we're fiddling about, go get the albums
		dispatch(getArtistAlbums(uri));

		// get both the artist and the top tracks
		$.when(

	        sendRequest( false, 'artists/'+id )
	            .then( response => {
	            	Object.assign(artist, response);
	            }),

	        sendRequest( false, 'artists/'+id+'/top-tracks?country='+getState().spotify.country )
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
        sendRequest( false, 'artists/'+ uri.replace('spotify:artist:','') +'/albums' )
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

        sendRequest( false, 'albums/'+ uri.replace('spotify:album:','') )
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

        dispatch({ type: 'SPOTIFY_LIBRARY_TRACKS_LOADED', data: false });

        sendRequest( getState().spotify.access_token, 'me/tracks' )
            .then( response => {
                dispatch({
                	type: 'SPOTIFY_LIBRARY_TRACKS_LOADED',
                	data: response
                });
            });
	}
}


var coreActions = require('../../services/core/actions')
var uiActions = require('../../services/ui/actions')
var mopidyActions = require('../../services/mopidy/actions')
var lastfmActions = require('../../services/lastfm/actions')
var helpers = require('../../helpers')

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

                // prepend the API baseurl, unless the endpoint already has it (ie pagination requests)
                var url = 'https://api.spotify.com/v1/'+endpoint
                if( endpoint.startsWith('https://api.spotify.com/') ) url = endpoint;

                // create our ajax request config
                var config = {
                    method: method,
                    url: url,
                    cached: true,
                    headers: {
                        Authorization: 'Bearer '+ response
                    }
                }

                // only if we've got data do we add it to the request (this prevents appending of "&false" to the URL)
                if (data){
                    if (typeof(data) === 'string'){
                        config.data = data
                    } else {
                        config.data = JSON.stringify(data)
                    }
                }

                // add reference to loader queue
                var loader_key = helpers.generateGuid()
                dispatch(uiActions.startLoading(loader_key, 'spotify_'+endpoint))

                $.ajax(config).then( 
                        response => {
                            dispatch(uiActions.stopLoading(loader_key))                            
                            resolve(response)
                        },
                        (xhr, status, error) => {
                            dispatch(uiActions.stopLoading(loader_key))
                            dispatch(coreActions.handleException(
                                'Spotify: '+xhr.responseJSON.error.message,
                                {
                                    source: 'spotify/actions.js/sendRequest',
                                    config: config,
                                    xhr: xhr,
                                    status: status,
                                    error: error
                                }
                            ))

                            // TODO: Instead of allowing request to fail before renewing the token, once refreshed
                            // we should retry the original request(s)
                            if (xhr.responseJSON.error.message == 'The access token expired'){
                                dispatch(refreshToken(dispatch, getState))
                            }

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
        if (getState().spotify.token_expiry && new Date().getTime() < getState().spotify.token_expiry){
            resolve(getState().spotify.access_token)
            return
        }

        // token is expiring/expired, so go get a new one and resolve that
        refreshToken(dispatch, getState)
            .then(
                response => {
                    resolve(response.access_token)
                }
            );
    });
}

function refreshToken( dispatch, getState ){
    return new Promise( (resolve, reject) => {

        if (getState().spotify.authorization){

            var config = {
                method: 'GET',
                url: getState().spotify.authorization_url+'?action=refresh&refresh_token='+getState().spotify.refresh_token,
                dataType: "json",
                timeout: 10000
            };

            $.ajax(config)
                .then(
                    response => {
                        response.token_expiry = new Date().getTime() + ( response.expires_in * 1000 )
                        response.source = 'spotify'
                        dispatch({
                            type: 'SPOTIFY_TOKEN_REFRESHED',
                            data: response
                        })
                        resolve(response)
                    },
                    (xhr, status, error) => {
                        dispatch({ type: 'SPOTIFY_DISCONNECTED' })
                        dispatch(coreActions.handleException(
                            'Spotify: '+xhr.responseJSON.error_description,
                            {
                                source: 'spotify/actions.js/refreshToken',
                                config: config,
                                xhr: xhr,
                                status: status,
                                error: error
                            }
                        ))
                        reject(error)
                    }
                );

        } else {

            $.ajax({
                    method: 'GET',
                    url: '//'+getState().mopidy.host+':'+getState().mopidy.port+'/iris/http/refresh_spotify_token',
                    dataType: "json",
                    timeout: 10000
                })
                .then(
                    response => {
                        if (response.type == 'error'){
                            dispatch({ type: 'SPOTIFY_DISCONNECTED' })
                            dispatch(coreActions.handleException(
                                'Spotify: '+response.message,
                                {
                                    source: 'spotify/actions.js/refreshToken',
                                    config: config,
                                    xhr: xhr,
                                    status: status,
                                    error: error
                                }
                            ))
                            reject(response)

                        } else {
                            var token = response.spotify_token
                            token.token_expiry = new Date().getTime() + ( token.expires_in * 1000 );
                            token.source = 'mopidy';
                            dispatch({
                                type: 'SPOTIFY_TOKEN_REFRESHED',
                                access_token_provider: 'backend',
                                data: token
                            });
                            resolve(token);                            
                        }

                    },
                    error => {
                        dispatch({ type: 'SPOTIFY_DISCONNECTED' })
                        dispatch(coreActions.handleException(
                            'Spotify: Could not refresh token',
                            {
                                source: 'spotify/actions.js/refreshToken',
                                config: config,
                                xhr: xhr,
                                status: status,
                                error: error
                            }
                        ))
                        reject(error)
                    }
                );
        }

    })
}

export function set(data){
    return {
        type: 'SPOTIFY_SET',
        data: data
    }
}

export function connect(){
    return (dispatch, getState) => {

        dispatch({ type: 'SPOTIFY_CONNECTING' });

        // send a generic request to ensure spotify is up and running
        // there is no 'test' or 'ping' endpoint on the Spotify API
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

export function authorizationGranted( data ){
    data.token_expiry = new Date().getTime() + data.expires_in;
    return { type: 'SPOTIFY_AUTHORIZATION_GRANTED', data: data }
}

export function revokeAuthorization(){
    return { type: 'SPOTIFY_AUTHORIZATION_REVOKED' }
}

export function refreshingToken(){
    return (dispatch, getState) => {
        dispatch({ type: 'SPOTIFY_TOKEN_REFRESHING' });
        refreshToken( dispatch, getState );
    }
}

export function importAuthorization(data){
    return {
        type: 'SPOTIFY_IMPORT_AUTHORIZATION',
        user: data.user,
        authorization: data.authorization
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
                    let track = Object.assign(
                        {},
                        response,
                        {
                            images: response.album.images
                        }
                    )
                    dispatch({
                        type: 'TRACK_LOADED',
                        key: uri,
                        track: track
                    });
                }
            );
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

        sendRequest( dispatch, getState, 'browse/featured-playlists?timestamp='+timestamp+'&country='+getState().core.country+'&limit=50&locale='+getState().core.locale )
            .then( response => {

                var playlists = []
                for (var i = 0; i < response.playlists.items.length; i++){
                    playlists.push(Object.assign(
                        {},
                        response.playlists.items[i],
                        {
                            can_edit: (getState().spotify.me && response.playlists.items[i].owner.id == getState().spotify.me.id),
                            tracks_total: response.playlists.items[i].tracks.total
                        }
                    ))
                }

                // Pick the first playlist, and get the full playlist object
                // We use this as in our introduction parallax panel, and need the full playlist
                dispatch(getPlaylist(playlists[0].uri))

                dispatch({
                    type: 'PLAYLISTS_LOADED',
                    playlists: playlists
                });

                dispatch({
                    type: 'SPOTIFY_FEATURED_PLAYLISTS_LOADED',
                    data: {
                        message: response.message,
                        playlists: helpers.arrayOf('uri',response.playlists.items)
                    }
                });
            });
    }
}

export function getCategories(){
    return (dispatch, getState) => {
        sendRequest( dispatch, getState, 'browse/categories?limit=50&country='+getState().core.country+'&locale='+getState().core.locale )
            .then( response => {
                dispatch({
                    type: 'CATEGORIES_LOADED',
                    categories: response.categories.items
                });
            });
    }
}

export function getCategory( id ){
    return (dispatch, getState) => {

        dispatch({
            type: 'CATEGORY_LOADED',
            key: 'category:'+id,
            category: {
                playlists_uris: null
            }
        });

        // get the category
        sendRequest( dispatch, getState, 'browse/categories/'+id+'?country='+getState().core.country+'&locale='+getState().core.locale )
            .then( response => {
                var category = Object.assign({}, response)
                dispatch({
                    type: 'CATEGORY_LOADED',
                    key: 'category:'+id,
                    category: Object.assign({}, response)
                });
            })

        // and the category's playlists
        sendRequest( dispatch, getState, 'browse/categories/'+id+'/playlists?limit=50&country='+getState().core.country+'&locale='+getState().core.locale )
            .then( response => {

                var playlists = []
                for (var i = 0; i < response.playlists.items.length; i++){
                    playlists.push(Object.assign(
                        {},
                        response.playlists.items[i],
                        {
                            tracks: null,
                            tracks_more: null,
                            tracks_total: response.playlists.items[i].tracks.total
                        }
                    ))
                }

                dispatch({
                    type: 'PLAYLISTS_LOADED',
                    playlists: playlists
                });

                dispatch({
                    type: 'SPOTIFY_CATEGORY_PLAYLISTS_LOADED',
                    key: 'category:'+id,
                    data: response
                });                
            })
    }
}

export function getNewReleases(){
    return (dispatch, getState) => {
        sendRequest( dispatch, getState, 'browse/new-releases?country='+getState().core.country+'&limit=50' )
            .then( response => {
                dispatch({
                    type: 'SPOTIFY_NEW_RELEASES_LOADED',
                    data: response
                });
            });
    }
}

export function getURL( url, action_name, key = false ){
    return (dispatch, getState) => {
        sendRequest( dispatch, getState, url )
            .then( response => {
                dispatch({
                    type: action_name,
                    key: key,
                    data: response
                });
            });
    }
}

export function clearSearchResults(){
    return {
        type: 'SPOTIFY_CLEAR_SEARCH_RESULTS'
    }
}

export function getSearchResults(type, query, limit = 50, offset = 0){
    return (dispatch, getState) => {

        dispatch(uiActions.startProcess('SPOTIFY_GET_SEARCH_RESULTS_PROCESSOR','Searching Spotify'))

        type = type.replace(/s+$/, "")
        if (type == 'all'){
            type = 'album,artist,playlist,track'
        }

        var url = 'search?q='+query
        url += '&type='+type
        url += '&country='+getState().core.country
        url += '&limit='+limit
        url += '&offset='+offset

        sendRequest( dispatch, getState, url )
            .then( response => {
                
                if (response.tracks !== undefined){
                    dispatch({
                        type: 'SPOTIFY_SEARCH_RESULTS_LOADED',
                        context: 'tracks',
                        results: response.tracks.items,
                        more: response.tracks.next,
                    });
                }
                
                if (response.artists !== undefined){
                    dispatch({
                        type: 'ARTISTS_LOADED',
                        artists: response.artists.items
                    });
                    dispatch({
                        type: 'SPOTIFY_SEARCH_RESULTS_LOADED',
                        context: 'artists',
                        results: helpers.arrayOf('uri',response.artists.items),
                        more: response.artists.next,
                    });
                }
                
                if (response.albums !== undefined){
                    dispatch({
                        type: 'ALBUMS_LOADED',
                        albums: response.albums.items
                    });
                    dispatch({
                        type: 'SPOTIFY_SEARCH_RESULTS_LOADED',
                        context: 'albums',
                        results: helpers.arrayOf('uri',response.albums.items),
                        more: response.albums.next,
                    });
                }

                if (response.playlists !== undefined){
                    var playlists = []
                    for (var i = 0; i < response.playlists.items.length; i++){
                        playlists.push(Object.assign(
                            {},
                            response.playlists.items[i],
                            {
                                can_edit: (getState().spotify.me && response.playlists.items[i].owner.id == getState().spotify.me.id),
                                tracks_total: response.playlists.items[i].tracks.total
                            }
                        ))
                    }
                    dispatch({
                        type: 'PLAYLISTS_LOADED',
                        playlists: playlists
                    });

                    dispatch({
                        type: 'SPOTIFY_SEARCH_RESULTS_LOADED',
                        context: 'playlists',
                        results: helpers.arrayOf('uri',playlists),
                        more: response.playlists.next
                    });
                }

                dispatch(uiActions.processFinished('SPOTIFY_GET_SEARCH_RESULTS_PROCESSOR'))
            });
    }
}

export function getAutocompleteResults(field_id, query, types = ['album','artist','playlist','track']){
    return (dispatch, getState) => {

        dispatch({type: 'SPOTIFY_AUTOCOMPLETE_LOADING', field_id: field_id})

        var genre_included = types.includes('genre')
        if (genre_included){
            var index = types.indexOf('genre')
            types.splice(index,1)
        }

        var endpoint = 'search?q='+query
        endpoint += '&type='+types.join(',')
        endpoint += '&country='+getState().core.country

        sendRequest(dispatch, getState, endpoint)
        .then(response => {
            var genres = []
            if (genre_included){
                var available_genres = getState().ui.genres
                if (available_genres){
                    for (var i = 0; i < available_genres.length; i++){
                        if (available_genres[i].includes(query)){
                            var genre = available_genres[i]
                            genres.push({
                                name: (genre.charAt(0).toUpperCase()+genre.slice(1)).replace('-',' '),
                                uri: 'spotify:genre:'+genre
                            })
                        }
                    }
                }
            }
            dispatch({
                type: 'SPOTIFY_AUTOCOMPLETE_LOADED',
                field_id: field_id,
                results: {
                    artists: (response.artists ? response.artists.items : []),
                    albums: (response.albums ? response.albums.items : []),
                    playlists: (response.playlists ? response.playlists.items : []),
                    tracks: (response.tracks ? response.tracks.items : []),
                    genres: genres
                }
            });
        })
    }
}

export function clearAutocompleteResults(field_id = null){
    return {
        type: 'SPOTIFY_AUTOCOMPLETE_CLEAR',
        field_id: field_id
    }
}

export function following(uri, method = 'GET'){
    return (dispatch, getState) => {

        if( method == 'PUT' ) var is_following = true
        if( method == 'DELETE' ) var is_following = false

        var asset_name = helpers.uriType(uri);
        var endpoint, data
        switch( asset_name ){
            case 'album':
                if( method == 'GET'){
                    endpoint = 'me/albums/contains/?ids='+ helpers.getFromUri('albumid', uri)
                }else{               
                    endpoint = 'me/albums/?ids='+ helpers.getFromUri('albumid', uri) 
                }
                break
            case 'artist':
                if( method == 'GET' ){
                    endpoint = 'me/following/contains?type=artist&ids='+ helpers.getFromUri('artistid', uri)   
                }else{
                    endpoint = 'me/following?type=artist&ids='+ helpers.getFromUri('artistid', uri)
                    data = {}                
                }
                break
            case 'user':
                if( method == 'GET' ){
                    endpoint = 'me/following/contains?type=user&ids='+ helpers.getFromUri('userid', uri)   
                }else{
                    endpoint = 'me/following?type=user&ids='+ helpers.getFromUri('userid', uri)
                    data = {}                
                }
                break
            case 'playlist':
                if( method == 'GET' ){
                    endpoint = 'users/'+ helpers.getFromUri('userid',uri) +'/playlists/'+ helpers.getFromUri('playlistid',uri) +'/followers/contains?ids='+ getState().spotify.me.id
                }else{
                    endpoint = 'users/'+ helpers.getFromUri('userid',uri) +'/playlists/'+ helpers.getFromUri('playlistid',uri) +'/followers'        
                }
                break
        }

        sendRequest( dispatch, getState, endpoint, method, data )
            .then( response => {
                if( response ) is_following = response
                if( typeof(is_following) === 'object' ) is_following = is_following[0]

                dispatch({
                    type: 'SPOTIFY_LIBRARY_'+asset_name.toUpperCase()+'_CHECK',
                    key: uri,
                    in_library: is_following
                })
            });
    }
}

/**
 * Resolve radio seeds into full objects
 *
 * @param radio object
 **/
export function resolveRadioSeeds( radio ){
    return (dispatch, getState) => {

        if (radio.seed_artists.length > 0){
            var artist_ids = '';
            for (var i = 0; i < radio.seed_artists.length; i++){
                if (i > 0) artist_ids += ','
                artist_ids += helpers.getFromUri('artistid', radio.seed_artists[i])
            }

            sendRequest( dispatch, getState, 'artists?ids='+ artist_ids )
            .then( response => {
                if (response && response.artists){
                    dispatch({
                        type: 'ARTISTS_LOADED',
                        artists: response.artists
                    })
                } else {
                    console.error('No Spotify artists returned', artist_ids)
                }
            })
        }

        if (radio.seed_tracks.length > 0){
            var track_ids = '';
            for (var i = 0; i < radio.seed_tracks.length; i++){
                if (i > 0) track_ids += ','
                track_ids += helpers.getFromUri('trackid', radio.seed_tracks[i])
            }
            
            sendRequest( dispatch, getState, 'tracks?ids='+ track_ids )
            .then( response => {
                dispatch({
                    type: 'TRACKS_LOADED',
                    tracks: response.tracks
                })
            })
        }
    }
}


/**
 * =============================================================== DISCOVER =============
 * ======================================================================================
 **/


/**
 * Get my favorites
 *
 * @param uri string
 **/
export function getFavorites(limit = 50, term = 'long_term'){
    return (dispatch, getState) => {

        dispatch({type: 'SPOTIFY_FAVORITES_LOADED', artists: [], tracks: []})

        $.when(
            sendRequest(dispatch, getState, 'me/top/artists?limit='+limit+'&time_range='+term),
            sendRequest(dispatch, getState, 'me/top/tracks?limit='+limit+'&time_range='+term)

        ).then((artists_response, tracks_response) => {
            dispatch({
                type: 'SPOTIFY_FAVORITES_LOADED',
                artists: artists_response.items,
                tracks: tracks_response.items
            });
        })
    }
}


/**
 * Get our recommendations
 * This is based off our 'favorites' and then we use those as seeds
 *
 * @param uris = array of artist or track URIs or a genre string
 **/
export function getRecommendations(uris = [], limit = 20){
    return (dispatch, getState) => {

        dispatch({type: 'SPOTIFY_RECOMMENDATIONS_LOADED', tracks: [], artists_uris: [], albums_uris: []})

        // build our starting point
        var artists_ids = []
        var tracks_ids = []
        var genres = []

        for (var i = 0; i < uris.length; i++){
            var uri = uris[i]

            switch (helpers.uriType(uri)){
                
                case 'artist':
                    artists_ids.push(helpers.getFromUri('artistid',uri))
                    break

                case 'track':
                    tracks_ids.push(helpers.getFromUri('trackid',uri))
                    break

                case 'genre':
                    genres.push(helpers.getFromUri('genreid',uri))
                    break

                case 'default':
                    genres.push(uri)
                    break
            }
        }

        // construct our endpoint URL with all the appropriate arguments
        var endpoint = 'recommendations'
        endpoint += '?seed_artists='+artists_ids.join(',')
        endpoint += '&seed_tracks='+tracks_ids.join(',')
        endpoint += '&seed_genres='+genres.join(',')
        endpoint += '&limit='+limit

        sendRequest(dispatch, getState, endpoint)
            .then( response => {

                // We only get simple artist objects, so we need to
                // get the full object. We'll add URIs to our recommendations
                // anyway so we can proceed in the meantime
                var artists_uris = []
                if (response.tracks.length > artists_ids.length && response.tracks.length > 10){
                    while (artists_uris.length < 5){
                        var random_index = Math.round(Math.random() * (response.tracks.length - 1))
                        var artist = response.tracks[random_index].artists[0]

                        // Make sure this artist is not already in our sample, and
                        // is not one of the seeds
                        if (!artists_uris.includes(artist.uri) && !artists_ids.includes(artist.id)){
                            artists_uris.push(artist.uri)
                            dispatch(getArtist(artist.uri))
                        }
                    }
                }

                // Copy already loaded albums into array
                var albums = []
                var albums_uris = []
                if (response.tracks.length > 10){
                    while (albums.length < 5){
                        var random_index = Math.round(Math.random() * (response.tracks.length - 1))
                        var album = response.tracks[random_index].album

                        // Make sure this album is not already in our sample
                        if (!albums_uris.includes(album.uri)){
                            albums_uris.push(album.uri)
                            albums.push(album)
                        }
                    }
                }

                // Officially add albums to index
                dispatch({
                    type: 'ALBUMS_LOADED',
                    albums: albums
                })

                dispatch({
                    type: 'SPOTIFY_RECOMMENDATIONS_LOADED',
                    seeds_uris: uris,
                    tracks: response.tracks,
                    artists_uris: artists_uris,
                    albums_uris: helpers.arrayOf('uri',albums)
                })
            })
    }
}


/**
 * Get all the available genres
 *
 * @param uri string
 **/
export function getGenres(){
    return (dispatch, getState) => {
        sendRequest(dispatch, getState, 'recommendations/available-genre-seeds')
            .then( response => {
                dispatch({
                    type: 'SPOTIFY_GENRES_LOADED',
                    genres: response.genres
                });
            })
    }
}




/**
 * =============================================================== ARTIST(S) ============
 * ======================================================================================
 **/

/**
 * Get a single artist
 *
 * @param uri string
 * @param full boolean (whether we want a full artist object)
 **/
export function getArtist(uri, full = false){
    return (dispatch, getState) => {

        // Start with an empty object
        // As each requests completes, they'll add to this object
        var artist = {}

        // We need our artist, obviously
        var requests = [        
            sendRequest( dispatch, getState, 'artists/'+ helpers.getFromUri('artistid', uri) )
            .then( response => {
                Object.assign(artist, response);
            })
        ]

        // Do we want a full artist, with all supporting material?
        if (full){
            requests.push(
                sendRequest( dispatch, getState, 'artists/'+ helpers.getFromUri('artistid', uri) +'/top-tracks?country='+getState().core.country )
                .then( response => {
                    Object.assign(artist, response);
                })
            )
            requests.push(
                sendRequest( dispatch, getState, 'artists/'+ helpers.getFromUri('artistid', uri) +'/related-artists' )
                .then( response => {
                    dispatch({
                        type: 'ARTISTS_LOADED',
                        artists: response.artists
                    }); 
                    Object.assign(artist, { related_artists_uris: helpers.arrayOf('uri',response.artists) });
                })
            )
        }

        // Run our requests
        $.when.apply($, requests).then(() => {

            if (artist.musicbrainz_id){
                dispatch(lastfmActions.getArtist(artist.uri, false, artist.musicbrainz_id))
            } else {
                dispatch(lastfmActions.getArtist(artist.uri, artist.name.replace('&','and')))
            }

            dispatch({
                type: 'ARTIST_LOADED',
                key: artist.uri,
                artist: artist
            })

            // Now go get our artist albums
            if (full){
                sendRequest( dispatch, getState, 'artists/'+ helpers.getFromUri('artistid', uri) +'/albums?market='+getState().core.country )
                .then( response => {
                    dispatch({
                        type: 'SPOTIFY_ARTIST_ALBUMS_LOADED',
                        data: response,
                        key: uri
                    })
                })
            }
        })
    }
}

export function getArtists( uris ){
    return (dispatch, getState) => {

        // now get all the artists for this album (full objects)
        var ids = '';
        for( var i = 0; i < uris.length; i++ ){
            if( ids != '' ) ids += ','
            ids += helpers.getFromUri( 'artistid', uris[i] );
        }

        sendRequest( dispatch, getState, 'artists/?ids='+ids )
            .then( response => {
                for (var i = i; i < response.length; i++){
                    var artist = response
                    for (var i = 0; i < artist.albums.length; i++){
                        dispatch({
                            type: 'ALBUM_LOADED',
                            album: artist.albums[i]
                        }); 
                    }
                    artist.albums = helpers.arrayOf('uri',artist.albums)
                    artist.albums_more = artist.albums.next
                    dispatch({
                        type: 'ARTIST_LOADED',
                        artist: artist
                    });                    
                }
            });
    }
}


export function playArtistTopTracks(uri){
    return (dispatch, getState) => {
        const artists = getState().core.artists

        // Do we have this artist (and their tracks) in our index already?
        if (typeof(artists[uri]) !== 'undefined' && typeof(artists[uri].tracks) !== 'undefined'){
            const uris = helpers.arrayOf('uri',artists[uri].tracks)
            dispatch(mopidyActions.playURIs(uris, uri))

        // We need to load the artist's top tracks first
        } else {
            sendRequest( dispatch, getState, 'artists/'+ helpers.getFromUri('artistid', uri) +'/top-tracks?country='+getState().core.country )
            .then( response => {
                const uris = helpers.arrayOf('uri',response.tracks)
                dispatch(mopidyActions.playURIs(uris, uri))
            })
        }
    }
}



/**
 * =============================================================== USER(S) ==============
 * ======================================================================================
 **/

export function getUser(uri){
    return (dispatch, getState) => {

        // get the user
        sendRequest( dispatch, getState, 'users/'+ helpers.getFromUri('userid',uri) )
            .then( response => {
                dispatch({
                    type: 'USER_LOADED',
                    key: response.uri,
                    user: response
                });
            })

        dispatch(getUserPlaylists(uri))
    }
}

export function getUserPlaylists(user_uri){
    return (dispatch, getState) => {

        // get the first page of playlists
        sendRequest( dispatch, getState, 'users/'+ helpers.getFromUri('userid', user_uri) +'/playlists?limit=40' )
            .then( response => {

                var playlists = []
                for (var i = 0; i < response.items.length; i++){

                    var can_edit = false
                    if (getState().spotify.me && response.items[i].owner.id == getState().spotify.me.id){
                        can_edit = true
                    } else if (response.items[i].owner.id == getState().backend_username){
                        can_edit = true
                    }

                    playlists.push(Object.assign(
                        {},
                        response.items[i],
                        {
                            can_edit: can_edit,
                            tracks_total: response.items[i].tracks.total
                        }
                    ))
                }

                dispatch({
                    type: 'PLAYLISTS_LOADED',
                    playlists: playlists
                });

                dispatch({
                    type: 'SPOTIFY_USER_PLAYLISTS_LOADED',
                    key: user_uri,
                    data: response
                });
            })
    }
}




/**
 * =============================================================== ALBUM(S) =============
 * ======================================================================================
 **/

/**
 * Single album
 *
 * @oaram uri string
 **/
export function getAlbum( uri ){
    return (dispatch, getState) => {

        // get the album
        sendRequest( dispatch, getState, 'albums/'+ helpers.getFromUri('albumid', uri) )
            .then( response => {

                // dispatch our loaded artists (simple objects)
                dispatch({
                    type: 'ARTISTS_LOADED',
                    artists: response.artists
                });

                var album = Object.assign(
                    {},
                    response,
                    {
                        artists_uris: helpers.arrayOf('uri',response.artists),
                        tracks: response.tracks.items,
                        tracks_more: response.tracks.next,
                        tracks_total: response.tracks.total
                    }
                )

                // add our album to all the tracks
                for (var i = 0; i < album.tracks.length; i++){
                    album.tracks[i].album = {
                        name: album.name,
                        uri: album.uri
                    }
                }

                dispatch({
                    type: 'ALBUM_LOADED',
                    key: album.uri,
                    album: album
                });

                // now get all the artists for this album (full objects)
                // we do this to get the artist artwork
                var artist_ids = [];
                for( var i = 0; i < response.artists.length; i++ ){
                    artist_ids.push( helpers.getFromUri( 'artistid', response.artists[i].uri ) )
                }

                // get all album artists as full objects
                sendRequest( dispatch, getState, 'artists/?ids='+artist_ids )
                    .then( response => {
                        dispatch({
                            type: 'ARTISTS_LOADED',
                            artists: response.artists
                        });
                    });

            })
    }
}

export function toggleAlbumInLibrary( uri, method ){
    if( method == 'PUT' ) var new_state = 1
    if( method == 'DELETE' ) var new_state = 0

    return (dispatch, getState) => {
        sendRequest( dispatch, getState, 'me/albums?ids='+ helpers.getFromUri('albumid',uri), method )
            .then( response => {
                dispatch({
                    type: 'SPOTIFY_ALBUM_FOLLOWING',
                    key: uri,
                    data: new_state
                });
            });
    }
}





/**
 * =============================================================== PLAYLIST(S) ==========
 * ======================================================================================
 **/

export function createPlaylist( name, description, is_public, is_collaborative ){
    return (dispatch, getState) => {

        var data = {
            name: name,
            description: description, 
            public: is_public,
            collaborative: is_collaborative
        }

        sendRequest(dispatch, getState, 'users/'+ getState().spotify.me.id +'/playlists/', 'POST', data)
        .then( response => {

            dispatch({
                type: 'PLAYLIST_LOADED',
                key: response.uri,
                playlist: Object.assign(
                    {},
                    response,
                    {
                        can_edit: true,
                        tracks: [],
                        tracks_more: null,
                        tracks_total: 0
                    })
            });

            dispatch({
                type: 'LIBRARY_PLAYLISTS_LOADED',
                uris: [response.uri]
            })

            dispatch(uiActions.createNotification('Created playlist'))
        })
    }
}

export function savePlaylist(uri, name, description, is_public, is_collaborative){
    return (dispatch, getState) => {

        var data = {
            name: name,
            description: description, 
            public: is_public,
            collaborative: is_collaborative
        }

        sendRequest( dispatch, getState, 'users/'+ getState().spotify.me.id +'/playlists/'+ helpers.getFromUri('playlistid',uri), 'PUT', data)
        .then( response => {
            dispatch({
                type: 'PLAYLIST_UPDATED',
                key: uri,
                playlist: {
                    name: name,
                    public: is_public,
                    collaborative: is_collaborative,
                    description: description
                }
            })
            dispatch(uiActions.createNotification('Saved'))
        })
    }
}

export function getPlaylist(uri){
    return (dispatch, getState) => {

        // get the main playlist object
        sendRequest( dispatch, getState, 'users/'+ helpers.getFromUri('userid',uri) +'/playlists/'+ helpers.getFromUri('playlistid',uri) +'?market='+getState().core.country )
        .then( response => {

            // convert links in description
            var description = null
            if (response.description){
                description = response.description
                description = description.split('<a href="spotify:artist:').join('<a href="#'+global.baseURL+'artist/spotify:artist:')
                description = description.split('<a href="spotify:album:').join('<a href="#'+global.baseURL+'album/spotify:album:')
                description = description.split('<a href="spotify:user:').join('<a href="#'+global.baseURL+'user/spotify:user:')
            }

            var playlist = Object.assign(
                {},
                response,
                {
                    can_edit: (getState().spotify.me && response.owner.id == getState().spotify.me.id),
                    tracks: helpers.flattenTracks(response.tracks.items),
                    tracks_more: response.tracks.next,
                    tracks_total: response.tracks.total,
                    description: description
                }
            )

            dispatch({
                type: 'PLAYLIST_LOADED',
                key: playlist.uri,
                playlist: playlist
            })
        })
    }
}

/**
 * Get all tracks for a playlist
 *
 * Recursively get .next until we have all tracks
 **/

export function getPlaylistTracksForPlaying(uri){
    return (dispatch, getState) => {
        dispatch(uiActions.startProcess(
            'SPOTIFY_GET_PLAYLIST_TRACKS_FOR_PLAYING_PROCESSOR',
            'Loading playlist tracks', 
            {
                uri: uri,
                next: 'users/'+ helpers.getFromUri('userid',uri) +'/playlists/'+ helpers.getFromUri('playlistid',uri) +'/tracks?market='+getState().core.country
            }
        ))
    }
}

export function getPlaylistTracksForPlayingProcessor(data){
    return (dispatch, getState) => {
        sendRequest(dispatch, getState, data.next)
            .then( response => {

                // Check to see if we've been cancelled
                if (getState().ui.processes['SPOTIFY_GET_PLAYLIST_TRACKS_FOR_PLAYING_PROCESSOR'] !== undefined){
                    var processor = getState().ui.processes['SPOTIFY_GET_PLAYLIST_TRACKS_FOR_PLAYING_PROCESSOR']

                    if (processor.status == 'cancelling'){
                        dispatch(uiActions.processCancelled('SPOTIFY_GET_PLAYLIST_TRACKS_FOR_PLAYING_PROCESSOR'))
                        return false
                    }
                }

                // Add on our new batch of loaded tracks
                var uris = []
                var new_uris = []
                for (var i = 0; i < response.items.length; i++){
                    new_uris.push(response.items[i].track.uri)
                }
                if (data.uris){
                    uris = [...data.uris, ...new_uris];
                } else {
                    uris = new_uris;
                }

                // We got a next link, so we've got more work to be done
                if (response.next){
                    dispatch(uiActions.updateProcess(
                        'SPOTIFY_GET_PLAYLIST_TRACKS_FOR_PLAYING_PROCESSOR', 
                        'Loading '+(response.total-uris.length)+' playlist tracks', 
                        {
                            next: response.next,
                            total: response.total,
                            remaining: response.total - uris.length
                        }
                    ))
                    dispatch(uiActions.runProcess(
                        'SPOTIFY_GET_PLAYLIST_TRACKS_FOR_PLAYING_PROCESSOR', 
                        {
                            next: response.next,
                            uris: uris
                        }
                    ))
                } else {
                    dispatch(mopidyActions.playURIs(uris, data.uri))
                    dispatch(uiActions.processFinished('SPOTIFY_GET_PLAYLIST_TRACKS_FOR_PLAYING_PROCESSOR'))
                }
            });
    }
}

export function toggleFollowingPlaylist(uri, method){
    if( method == 'PUT' ) var new_state = 1
    if( method == 'DELETE' ) var new_state = 0

    return (dispatch, getState) => {
        sendRequest( dispatch, getState, 'users/'+ helpers.getFromUri('userid',uri) + '/playlists/'+ helpers.getFromUri('playlistid',uri) + '/followers', method )
            .then( response => {
                dispatch({
                    type: 'SPOTIFY_PLAYLIST_FOLLOWING_LOADED',
                    key: uri,
                    is_following: new_state
                });
            });
    }
}

export function addTracksToPlaylist( uri, tracks_uris ){
    return (dispatch, getState) => {
        sendRequest( dispatch, getState, 'users/'+ helpers.getFromUri('userid',uri) + '/playlists/'+ helpers.getFromUri('playlistid',uri) + '/tracks', 'POST', { uris: tracks_uris } )
            .then( response => {
                dispatch({
                    type: 'PLAYLIST_TRACKS_ADDED',
                    key: uri,
                    tracks_uris: tracks_uris,
                    snapshot_id: response.snapshot_id
                });
            });
    }
}

export function deleteTracksFromPlaylist( uri, snapshot_id, tracks_indexes ){
    return (dispatch, getState) => {
        sendRequest( dispatch, getState, 'users/'+ helpers.getFromUri('userid',uri) + '/playlists/'+ helpers.getFromUri('playlistid',uri) + '/tracks', 'DELETE', { snapshot_id: snapshot_id, positions: tracks_indexes } )
            .then( response => {
                dispatch({
                    type: 'PLAYLIST_TRACKS_REMOVED',
                    key: uri,
                    tracks_indexes: tracks_indexes,
                    snapshot_id: response.snapshot_id
                });
            });
    }
}

export function reorderPlaylistTracks( uri, range_start, range_length, insert_before, snapshot_id ){
    return (dispatch, getState) => {
        sendRequest( dispatch, getState, 'users/'+ helpers.getFromUri('userid',uri) + '/playlists/'+ helpers.getFromUri('playlistid',uri) + '/tracks', 'PUT', { uri: uri, range_start: range_start, range_length: range_length, insert_before: insert_before, snapshot_id: snapshot_id } )
            .then( response => {
                dispatch({
                    type: 'PLAYLIST_TRACKS_REORDERED',
                    key: uri,
                    range_start: range_start,
                    range_length: range_length,
                    insert_before: insert_before,
                    snapshot_id: response.snapshot_id
                });
            });
    }
}



/**
 * =============================================================== LIBRARY ==============
 * ======================================================================================
 **/
 

/**
 * Playlists
 **/

export function getLibraryPlaylists(){
    return (dispatch, getState) => {
        var last_run = getState().ui.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR

        if (!last_run){
            dispatch(uiActions.startProcess('SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR','Loading Spotify playlists', {next: 'me/playlists?limit=50'}))    
        } else if (last_run.status == 'cancelled'){
            dispatch(uiActions.resumeProcess('SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR'))     
        } else if (last_run.status == 'finished'){
            // TODO: do we want to force a refresh?   
        }
    }
}

export function getLibraryPlaylistsProcessor(data){
    return (dispatch, getState) => {
        sendRequest(dispatch, getState, data.next)
            .then( response => {

                dispatch({
                    type: 'SPOTIFY_LIBRARY_PLAYLISTS_LOADED',
                    playlists: response.items
                })

                // Check to see if we've been cancelled
                if (getState().ui.processes['SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR'] !== undefined){
                    var processor = getState().ui.processes['SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR']

                    if (processor.status == 'cancelling'){
                        dispatch(uiActions.processCancelled('SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR'))
                        return false
                    }
                }

                // We got a next link, so we've got more work to be done
                if (response.next){
                    var total = response.total
                    var loaded = getState().spotify.library_playlists.length
                    var remaining = total - loaded
                    dispatch(uiActions.updateProcess(
                        'SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR', 
                        'Loading '+remaining+' Spotify playlists', 
                        {
                            next: response.next,
                            total: response.total,
                            remaining: remaining
                        }
                    ))
                    dispatch(uiActions.runProcess('SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR', {next: response.next}))
                } else {
                    dispatch(uiActions.processFinished('SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR'))
                    dispatch({type: 'SPOTIFY_LIBRARY_PLAYLISTS_LOADED_ALL'})
                }
            });
    }
}
 

/**
 * Artists
 **/

export function getLibraryArtists(){
    return (dispatch, getState) => {
        var last_run = getState().ui.processes.SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR

        if (!last_run){
            dispatch(uiActions.startProcess('SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR','Loading Spotify artists', {next: 'me/following?type=artist&limit=50'}))
        } else if (last_run.status == 'cancelled'){
            dispatch(uiActions.resumeProcess('SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR'))     
        } else if (last_run.status == 'finished'){
            // TODO: do we want to force a refresh?   
        }
    }
}

export function getLibraryArtistsProcessor(data){
    return (dispatch, getState) => {
        sendRequest(dispatch, getState, data.next)
            .then( response => {

                dispatch({
                    type: 'SPOTIFY_LIBRARY_ARTISTS_LOADED',
                    artists: response.artists.items
                })

                // Check to see if we've been cancelled
                if (getState().ui.processes['SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR'] !== undefined){
                    var processor = getState().ui.processes['SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR']

                    if (processor.status == 'cancelling'){
                        dispatch(uiActions.processCancelled('SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR'))
                        return false
                    }
                }

                // We got a next link, so we've got more work to be done
                if (response.artists.next){
                    var total = response.artists.total
                    var loaded = getState().spotify.library_artists.length
                    var remaining = total - loaded
                    dispatch(uiActions.updateProcess(
                        'SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR', 
                        'Loading '+remaining+' Spotify artists', 
                        {
                            next: response.artists.next, 
                            total: response.artists.total,
                            remaining: remaining
                        }
                    ))
                    dispatch(uiActions.runProcess('SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR', {next: response.artists.next}))
                } else {
                    dispatch(uiActions.processFinished('SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR'))
                }
            });
    }
}
 

/**
 * ALbums
 **/

export function getLibraryAlbums(){
    return (dispatch, getState) => {
        var last_run = getState().ui.processes.SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR

        if (!last_run){
            dispatch(uiActions.startProcess('SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR','Loading Spotify albums', {next: 'me/albums?limit=50'}))        
        } else if (last_run.status == 'cancelled'){
            dispatch(uiActions.updateProcess('SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR','Loading Spotify albums', {next: 'me/albums?limit=50'}))     
        } else if (last_run.status == 'finished'){
            // TODO: do we want to force a refresh?   
        }
    }
}

export function getLibraryAlbumsProcessor(data){
    return (dispatch, getState) => {
        sendRequest(dispatch, getState, data.next)
            .then( response => {

                dispatch({
                    type: 'SPOTIFY_LIBRARY_ALBUMS_LOADED',
                    albums: response.items
                })

                // Check to see if we've been cancelled
                if (getState().ui.processes['SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR'] !== undefined){
                    var processor = getState().ui.processes['SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR']

                    if (processor.status == 'cancelling'){
                        dispatch(uiActions.processCancelled('SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR'))
                        return false
                    }
                }

                // We got a next link, so we've got more work to be done
                if (response.next){
                    var total = response.total
                    var loaded = getState().spotify.library_albums.length
                    var remaining = total - loaded
                    dispatch(uiActions.updateProcess(
                        'SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR', 
                        'Loading '+remaining+' Spotify albums', 
                        {
                            next: response.next, 
                            total: response.total,
                            remaining: remaining
                        }
                    ))
                    dispatch(uiActions.runProcess('SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR', {next: response.next}))
                } else {
                    dispatch(uiActions.processFinished('SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR'))
                }
            });
    }
}
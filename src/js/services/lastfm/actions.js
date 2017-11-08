
var coreActions = require('../core/actions')
var uiActions = require('../ui/actions')
var helpers = require('../../helpers')

/**
 * Send an ajax request to the LastFM API
 *
 * @param dispatch = obj
 * @param getState = obj
 * @param params = string, the url params to send
 * @params signed = boolean, whether we've got a signed request with baked-in api_key
 **/
const sendRequest = (dispatch, getState, params, signed = false) => {
    return new Promise((resolve, reject) => {

        var loader_key = helpers.generateGuid();
        var method = params.substring(params.indexOf("method=")+7, params.length);
        method = method.substring(0, method.indexOf("&"));

        dispatch(uiActions.startLoading(loader_key, 'lastfm_'+method));

        var config = {
            method: 'GET',
            cache: true,
            timeout: 30000,
            url: '//ws.audioscrobbler.com/2.0/?format=json&'+params
        }

        // Signed requests don't need our api_key as the proxy has it's own 
        if (!signed){
            config.url += '&api_key=4320a3ef51c9b3d69de552ac083c55e3';
        } else {
            config.method = 'POST';
        }

        $.ajax(config).then(
                response => {
                    dispatch(uiActions.stopLoading(loader_key));
                    if (response.error){
                        reject({
                            config: config,
                            error: response
                        });
                    } else {
                        resolve(response);
                    }
                },
                (xhr, status, error) => {
                    dispatch(uiActions.stopLoading(loader_key));

                    // Snatch a more meaningful error
                    var description = null;
                    if (xhr.responseJSON.message){
                        description = xhr.responseJSON.message;
                    }

                    reject({
                        config: config,
                        error: error,
                        description: description,
                        status: status,
                        xhr: xhr
                    });
                }
            )
    })
}

/**
 * Send a SIGNED ajax request to the LastFM API
 *
 * @param dispatch = obj
 * @param getState = obj
 * @param params = string, the url params to send
 * @param signed = boolean
 **/
const sendSignedRequest = (dispatch, getState, params) => {
    return new Promise((resolve, reject) => {

        // Not authorized
        if (!getState().lastfm.session){
            reject({
                params: params,
                error: "No active LastFM session"
            });
        }

        var loader_key = helpers.generateGuid();
        var method = params.substring(params.indexOf("method=")+7, params.length);
        method = method.substring(0, method.indexOf("&"));

        dispatch(uiActions.startLoading(loader_key, 'lastfm_'+method));

        params += "&sk="+getState().lastfm.session.key;

        var config = {
            method: 'GET',
            cache: false,
            timeout: 30000,
            url: getState().lastfm.authorization_url+"?action=sign_request&"+params
        }

        // Get our server proxy to sign our request
        $.ajax(config).then(
            response => {
                dispatch(uiActions.stopLoading(loader_key));

                // Now we have signed params, we can make the actual request
                sendRequest(dispatch, getState, response.params, true)
                    .then(
                        response => {
                            resolve(response);
                        },
                        error => {
                            reject(error);
                        }
                    );
            },
            (xhr, status, error) => {
                dispatch(uiActions.stopLoading(loader_key));
                reject(error)
            }
        );
    })
}


export function set(data){
    return {
        type: 'LASTFM_SET',
        data: data
    }
}


/**
 * Handle authorization process
 **/

export function authorizationGranted(data){
    data.session.expiry = new Date().getTime() + 3600;
    return {
        type: 'LASTFM_AUTHORIZATION_GRANTED',
        data: data
    }
}

export function revokeAuthorization(){
    return { type: 'LASTFM_AUTHORIZATION_REVOKED' }
}

export function connect(){
    return (dispatch, getState) => {

        dispatch({ type: 'LASTFM_CONNECTING' });

        // Authorized? Multi-purpose our connection test to get the current user
        if (getState().lastfm.session){
            dispatch(getMe());

        // Not authorized? Just use a generic lookup to test our connection
        } else {
            sendRequest(dispatch, getState, 'method=artist.getInfo&artist=Moby')
                .then(
                    response => {
                        dispatch({ type: 'LASTFM_CONNECTED' })
                    },
                    error => {
                        dispatch({ type: 'LASTFM_DISCONNECTED' })
                    }
                )
        }
    }
}


/**
 * Signed requests
 * TODO
 **/

export function loveTrack(uri, artist, track){
    return (dispatch, getState) => {
        artist = encodeURIComponent(artist);
        var params = 'method=track.love&track='+track+'&artist='+artist;
        sendSignedRequest(dispatch, getState, params)
            .then(
                response => {
                    dispatch({
                        type: 'TRACK_LOADED',
                        key: uri,
                        track: {
                            userloved: true
                        }
                    });
                }
            )
    }
}

export function unloveTrack(uri, artist, track){
    return (dispatch, getState) => {
        artist = encodeURIComponent(artist);
        var params = 'method=track.unlove&track='+track+'&artist='+artist;
        sendSignedRequest(dispatch, getState, params)
            .then(
                response => {
                    dispatch({
                        type: 'TRACK_LOADED',
                        key: uri,
                        track: {
                            userloved: false
                        }
                    });
                }
            )
    }
}


/**
 * Non-signed requests
 **/

export function getMe(){
    return (dispatch, getState) => {
        var params = 'method=user.getInfo&user='+getState().lastfm.session.name
        sendRequest(dispatch, getState, params)
            .then(
                response => {
                    if (response.user){
                        dispatch({
                            type: 'LASTFM_USER_LOADED',
                            user: response.user
                        });
                        dispatch({ type: 'LASTFM_CONNECTED' })
                    }
                }
            )
    }
}

export function getArtist(uri, artist, mbid = false){
    return (dispatch, getState) => {
        if (mbid){
            var params = 'method=artist.getInfo&mbid='+mbid
        } else {
            artist = encodeURIComponent(artist );
            var params = 'method=artist.getInfo&artist='+artist
        }
        sendRequest(dispatch, getState, params)
            .then(
                response => {
                    if (response.artist){
                        dispatch({
                            type: 'ARTIST_LOADED',
                            key: uri,
                            artist: {
                                images: response.artist.image,
                                bio: response.artist.bio,
                                listeners: parseInt(response.artist.stats.listeners),
                                play_count: parseInt(response.artist.stats.playcount),
                                on_tour: response.artist.stats.ontour
                            }
                        });
                    }
                }
            )
    }
}

export function getAlbum(artist, album, mbid = false){
    return (dispatch, getState) => {

        dispatch({ type: 'LASTFM_ALBUM_LOADED', data: false });

        if (mbid){
            var params = 'method=album.getInfo&mbid='+mbid
        } else {
            artist = encodeURIComponent(artist )
            album = encodeURIComponent(album )
            var params = 'method=album.getInfo&album='+album+'&artist='+artist
        }
        sendRequest(dispatch, getState, params)
            .then(
                response => {
                    if (response.album){
                        dispatch({
                            type: 'LASTFM_ALBUM_LOADED',
                            data: response.album
                        });
                    }
                }
            )
    }
}

export function getTrack(track, artist_name = null, track_name = null){
    return (dispatch, getState) => {
        if (track){
            track_name = track.name;
            if (track.artists){
                artist_name = track.artists[0].name;
            }
        }
        artist_name = encodeURIComponent(artist_name);
        var params = 'method=track.getInfo&track='+track_name+'&artist='+artist_name;
        if (getState().lastfm.session){
            params += '&username='+getState().lastfm.session.name;
        }
        sendRequest(dispatch, getState, params)
            .then(
                response => {
                    if (response.track){
                        var merged_track = Object.assign(
                            {},
                            response.track,
                            track
                        );
                        dispatch({
                            type: 'TRACK_LOADED',
                            key: merged_track.uri,
                            track: merged_track
                        });
                    }
                }
            )
    }
}

export function scrobble(track){
    return (dispatch, getState) => {
        var track_name = track.name;
        var artist_name = "Unknown";
        if (track.artists){
            artist_name = track.artists[0].name;
        }
        var artist_name = encodeURIComponent(artist_name);

        var params = 'method=track.scrobble';
        params += '&track='+track_name+'&artist='+artist_name;
        params += '&timestamp='+Math.floor(Date.now() / 1000);

        sendSignedRequest(dispatch, getState, params)
            .then(
                response => {
                    console.log("Scrobbled", response);
                },
                error => {
                    dispatch(coreActions.handleException(
                        'Could not scrobble track',
                        error,
                        (error.description ? error.description : null)
                    ));
                }
            )
    }
}



var coreActions = require('../core/actions');
var uiActions = require('../ui/actions');
var helpers = require('../../helpers');


export function set(data){
    return {
        type: 'LASTFM_SET',
        data: data
    }
}

/**
 * Send an ajax request to the LastFM API
 *
 * @param dispatch = obj
 * @param getState = obj
 * @param params = string, the url params to send
 * @params signed = boolean, whether we've got a signed request with baked-in api_key
 **/
var sendRequest = (dispatch, getState, params, signed = false) => {
    return new Promise((resolve, reject) => {

        var loader_key = helpers.generateGuid();
        var method = params.substring(params.indexOf("method=")+7, params.length);
        method = method.substring(0, method.indexOf("&"));

        dispatch(uiActions.startLoading(loader_key, 'lastfm_'+method));

        var config = {
            method: 'GET',
            cache: true,
            timeout: 30000,
            url: 'https://ws.audioscrobbler.com/2.0/?format=json&'+params
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
    return {
    	type: 'LASTFM_AUTHORIZATION_REVOKED'
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
                            type: 'LASTFM_ME_LOADED',
                            me: response.user
                        });
                    }
                }
            )
    }
}

export function getTrack(uri){
    return (dispatch, getState) => {
        if (getState().core.tracks[uri] !== undefined){
            var track = getState().core.tracks[uri];
            if (!track.artists){
                dispatch(coreActions.handleException(
                    "Could not get track",
                    {},
                    "Track has no artists"
                ));
                return;
            }
        } else {
            dispatch(coreActions.handleException(
                "Could not get track",
                {},
                "Could not find track in index"
            ));
            return;
        }

        var track_name = track.name;
        var artist_name = encodeURIComponent(track.artists[0].name);
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
                            {
                                uri: track.uri
                            },
                            response.track,
                            track
                        );
                        dispatch(coreActions.trackLoaded(merged_track));
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
            artist = artist.replace("&","and");
            artist = encodeURIComponent(artist);
            var params = 'method=artist.getInfo&artist='+artist
        }
        sendRequest(dispatch, getState, params)
            .then(
                response => {
                    if (response.artist){
                    	var artist = {
                            uri: uri,
                            images: response.artist.image,
                            mbid: response.artist.mbid,
                            bio: response.artist.bio,
                            listeners: parseInt(response.artist.stats.listeners),
                            play_count: parseInt(response.artist.stats.playcount),
                            on_tour: response.artist.stats.ontour
                        };

                        dispatch(coreActions.artistLoaded(artist));
                    }
                }
            )
    }
}

export function getAlbum(artist, album, mbid = false){
    return (dispatch, getState) => {
        if (mbid){
            var params = 'method=album.getInfo&mbid='+mbid;
        } else {
            artist = encodeURIComponent(artist);
            album = encodeURIComponent(album);
            var params = 'method=album.getInfo&album='+album+'&artist='+artist;
        }
        sendRequest(dispatch, getState, params)
            .then(
                response => {
                    if (response.album){
                        dispatch(coreActions.albumLoaded(response.album));
                    }
                }
            );
    }
}


export function getImages(context, uri){
    return (dispatch, getState) => {

        var record = getState().core[context][uri];
        if (record){
            switch (context){

                case "tracks":

                    if (record.mbid){
                        var params = 'method=album.getInfo&mbid='+record.mbid;
                    } else if (record.artists && record.artists.length > 0 && record.album){
                        var artist = encodeURIComponent(record.artists[0].name);
                        var album = encodeURIComponent(record.album.name);
                        var params = 'method=album.getInfo&album='+album+'&artist='+artist;
                    }

                    if (params){
                        sendRequest(dispatch, getState, params)
                            .then(
                                response => {
                                    if (response.album){
                                        record = Object.assign({}, record, {images: response.album.image});
                                        dispatch(coreActions.trackLoaded(record));
                                        dispatch(coreActions.albumLoaded(response.album));
                                    }
                                }
                            );
                    }
                    break;

                case "albums":

                    if (record.mbid){
                        var params = 'method=album.getInfo&mbid='+record.mbid;
                    } else if (record.artists && record.artists.length > 0){
                        var artist = encodeURIComponent(record.artists[0].name);
                        var album = encodeURIComponent(record.name);
                        var params = 'method=album.getInfo&album='+album+'&artist='+artist;
                    }

                    if (params){
                        sendRequest(dispatch, getState, params)
                            .then(
                                response => {
                                    if (response.album){
                                        record = Object.assign({}, record, {images: response.album.image});
                                        dispatch(coreActions.albumLoaded(record));
                                    }
                                }
                            );
                    }
                    break;
            }
        }
    }
}







/**
 * Signed requests
 **/

export function loveTrack(uri){
    return (dispatch, getState) => {
        if (getState().core.tracks[uri] !== undefined){
            var track = getState().core.tracks[uri];
            if (!track.artists){
                dispatch(coreActions.handleException(
                    "Could not love track",
                    track,
                    "Track has no artists"
                ));
                return;
            }
        } else {
            dispatch(coreActions.handleException(
                "Could not love track",
                track,
                "Could not find track in index"
            ));
            return;
        }

        var artist = encodeURIComponent(track.artists[0].name);
        var params = 'method=track.love&track='+track.name+'&artist='+artist;
        sendSignedRequest(dispatch, getState, params)
            .then(
                response => {
                    track = Object.assign(
                        {},
                        track,
                        {
                            userloved: true
                        }
                    );
                    dispatch({
                        type: 'TRACKS_LOADED',
                        tracks: [track]
                    });
                }
            );
    }
}

export function unloveTrack(uri){
    return (dispatch, getState) => {
        if (getState().core.tracks[uri] !== undefined){
            var track = getState().core.tracks[uri];
            if (!track.artists){
                dispatch(coreActions.handleException(
                    "Could not unlove track",
                    track,
                    "Track has no artists"
                ));
                return;
            }
        } else {
            dispatch(coreActions.handleException(
                "Could not unlove track",
                track,
                "Could not find track in index"
            ));
            return;
        }

        var artist = encodeURIComponent(track.artists[0].name);
        var params = 'method=track.unlove&track='+track.name+'&artist='+artist;
        sendSignedRequest(dispatch, getState, params)
            .then(
                response => {
                    track = Object.assign(
                        {},
                        track,
                        {
                            userloved: false
                        }
                    );
                    dispatch({
                        type: 'TRACKS_LOADED',
                        tracks: [track]
                    });
                }
            );
    }
}

/**
 * TODO: Currently scrobbling client-side would result in duplicated scrobbles
 * if the user was authorized across multiple connections. Ideally this would
 * be handled server-side. Mopidy-Scrobbler currently achieves this.
 **/
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

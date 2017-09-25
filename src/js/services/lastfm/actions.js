
var coreActions = require('../core/actions')
var uiActions = require('../ui/actions')
var helpers = require('../../helpers')

/**
 * Send an ajax request to the Spotify API
 *
 * @param dispatch obj
 * @param getState obj
 * @param endpoint params = the url params to send
 **/
const sendRequest = ( dispatch, getState, params ) => {
    return new Promise( (resolve, reject) => {

        var loader_key = helpers.generateGuid()
        dispatch(uiActions.startLoading(loader_key, 'lastfm_'+params))

        var config = {
            method: 'GET',
            cache: true,
            url: '//ws.audioscrobbler.com/2.0/?format=json&api_key=4320a3ef51c9b3d69de552ac083c55e3&'+params
        }

        $.ajax(config).then( 
                response => {
                    dispatch(uiActions.stopLoading(loader_key))    
                    resolve(response)
                },
                (xhr, status, error) => {
                    dispatch(uiActions.stopLoading(loader_key))    
                    dispatch(coreActions.handleException(
                        'LastFM: '+xhr.responseText, 
                        {
                            config: config,
                            error: error,
                            status: status,
                            xhr: xhr
                        }
                    ));
                    reject(error)
                }
            )
    })
}

export function connect(){
    return (dispatch, getState) => {

        dispatch({ type: 'LASTFM_CONNECTING' })

        sendRequest(dispatch, getState, 'method=artist.getInfo&artist=')
            .then(
                response => {
                    dispatch({ type: 'LASTFM_CONNECTED' })
                }
            )
    }
}

export function getArtist( uri, artist, mbid = false ){
    return (dispatch, getState) => {
        if( mbid ){
            var params = 'method=artist.getInfo&mbid='+mbid
        }else{
            artist = encodeURIComponent( artist );
            var params = 'method=artist.getInfo&artist='+artist
        }
        sendRequest(dispatch, getState, params)
            .then(
                response => {
                    if( response.artist ){
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

export function getAlbum( artist, album, mbid = false ){
    return (dispatch, getState) => {

        dispatch({ type: 'LASTFM_ALBUM_LOADED', data: false });

        if( mbid ){
            var params = 'method=album.getInfo&mbid='+mbid
        }else{
            artist = encodeURIComponent( artist )
            album = encodeURIComponent( album )
            var params = 'method=album.getInfo&album='+album+'&artist='+artist
        }
        sendRequest(dispatch, getState, params)
            .then(
                response => {
                    if( response.album ){
                        dispatch({
                            type: 'LASTFM_ALBUM_LOADED',
                            data: response.album
                        });
                    }
                }
            )
    }
}

export function getTrack( artist, track ){
    return (dispatch, getState) => {

        dispatch({ type: 'LASTFM_TRACK_LOADED', data: false });
        
        artist = encodeURIComponent( artist );
        sendRequest(dispatch, getState, 'method=track.getInfo&track='+track+'&artist='+artist)
            .then(
                response => {
                    if( response.track ){
                        dispatch({
                            type: 'LASTFM_TRACK_LOADED',
                            data: response.track
                        });
                    }
                }
            )
    }
}


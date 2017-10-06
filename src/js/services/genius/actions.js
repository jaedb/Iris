
var coreActions = require('../core/actions')
var uiActions = require('../ui/actions')
var helpers = require('../../helpers')

/**
 * Send an ajax request
 *
 * @param dispatch obj
 * @param getState obj
 * @param endpoint params = the url params to send
 **/
const sendRequest = (dispatch, getState, endpoint) => {
    return new Promise( (resolve, reject) => {

        var loader_key = helpers.generateGuid();
        dispatch(uiActions.startLoading(loader_key, 'genius_'+endpoint));

        var config = {
            method: 'POST',
            cache: false,
            timeout: 15000,
            headers: {
                Authorization: 'Bearer 2AGP9sfzKQcxfKSZuGa_3lqsDIpuOiTGT7-vhJYcKaaDjHIIA2HICsxXCiC30Xxi'
            },
            data: JSON.stringify({
                url: 'https://api.genius.com/'+endpoint
            }),
            url: '//'+getState().mopidy.host+':'+getState().mopidy.port+'/iris/http/proxy_request'
        };

        $.ajax(config).then( 
            response => {
                dispatch(uiActions.stopLoading(loader_key));
                resolve(response.response);
            },
            (xhr, status, error) => {
                dispatch(uiActions.stopLoading(loader_key));
                reject({
                    config: config,
                    xhr: xhr,
                    status: status,
                    error: error
                });
            }
        )
    })
}

export function getTrackLyrics(track){
    return (dispatch, getState) => {

        var query = '';
        for (var i = 0; i < track.artists.length; i++){
            query += track.artists[i].name+' ';
        }
        query += track.name+' lyrics';
        query = query.replace(/\s+/g, '-').toLowerCase();

        var config = {
            method: 'POST',
            cache: false,
            timeout: 15000,
            data: JSON.stringify({
                url: 'https://genius.com/'+query
            }),
            url: '//'+getState().mopidy.host+':'+getState().mopidy.port+'/iris/http/proxy_request'
        };

        $.ajax(config).then( 
            response => {
                var html = $(response.response);
                var lyrics = html.find('.lyrics');
                if (lyrics.length > 0){

                    lyrics = lyrics.first();
                    lyrics.find('a').replaceWith(function(){ return this.innerHTML; });

                    dispatch({
                        type: 'TRACK_LOADED',
                        key: track.uri,
                        track: {
                            lyrics: lyrics.html()
                        }
                    });
                }
            },
            (xhr, status, error) => {
                console.log(xhr, status, error)
            }
        )
    }
}

export function getTrackInfo(track){
    return (dispatch, getState) => {

        var query = '';
        for (var i = 0; i < track.artists.length; i++){
            query += track.artists[i].name+' ';
        }
        query += track.name;

        sendRequest(dispatch, getState, 'search?q='+encodeURIComponent(query))
            .then(
                response => {
                    if (response.response.hits && response.response.hits.length > 0){
                        dispatch({
                            type: 'TRACK_LOADED',
                            key: track.uri,
                            track: {
                                annotations: response.response.hits[0].result
                            }
                        });
                    }
                },
                error => {
                    dispatch(coreActions.handleException(
                        'Could not get track info',
                        error
                    ));
                }
            );
    }
}

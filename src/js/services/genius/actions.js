
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
    return new Promise((resolve, reject) => {

        var loader_key = helpers.generateGuid();
        dispatch(uiActions.startLoading(loader_key, 'genius_'+endpoint));

        var config = {
            method: 'POST',
            cache: false,
            timeout: 30000,
            headers: {
                Authorization: 'Bearer 2AGP9sfzKQcxfKSZuGa_3lqsDIpuOiTGT7-vhJYcKaaDjHIIA2HICsxXCiC30Xxi'
            },
            data: JSON.stringify({
                url: 'https://genius.com/'+endpoint
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

        var endpoint = '';
        for (var i = 0; i < track.artists.length; i++){
            endpoint += track.artists[i].name+' ';
        }
        endpoint += track.name+' lyrics';
        endpoint = endpoint.replace(/\s+/g, '-').toLowerCase();

        sendRequest(dispatch, getState, endpoint)
            .then(
                response => {
                    var html = $(response);
                    var lyrics = html.find('.lyrics');
                    if (lyrics.length > 0){

                        lyrics = lyrics.first();
                        lyrics.find('a').replaceWith(function(){ 
                            return this.innerHTML;
                        });

                        var lyrics_html = lyrics.html();
                        lyrics_html = lyrics_html.replace(/(\[)/g, '<span class="grey-text">[');
                        lyrics_html = lyrics_html.replace(/(\])/g, ']</span>');

                        dispatch({
                            type: 'TRACK_LOADED',
                            key: track.uri,
                            track: {
                                lyrics: lyrics_html
                            }
                        });
                    }
                },
                error => {
                    dispatch(coreActions.handleException(
                        'Could not get track lyrics',
                        error
                    ));
                }
            );
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

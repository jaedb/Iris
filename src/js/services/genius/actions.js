
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

        var url = endpoint;
        if (!url.startsWith('http')){
            url = 'https://api.genius.com/'+url;
        }

        var config = {
            method: 'POST',
            cache: false,
            timeout: 30000,
            headers: {
                Authorization: 'Bearer nBNNEFekix8BOsfPyfK7LtX-CaUz7L7ak92qC3GfMAIQi8eWjuwb4P8SUxK1K-iY'
            },
            data: JSON.stringify({
                url: url
            }),
            url: '//'+getState().mopidy.host+':'+getState().mopidy.port+'/iris/http/proxy_request'
        };

        $.ajax(config).then(
            response => {
                dispatch(uiActions.stopLoading(loader_key));
                resolve(response.result);
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

/**
 * Extract lyrics from a page
 * We don't get the lyrics in the API, so we need to 'scrape' the HTML page instead
 *
 * @param uri = track uri
 * @param result = lyrics result (title, url)
 **/
export function getTrackLyrics(uri, url){
    return (dispatch, getState) => {

        dispatch({
            type: 'TRACK_LOADED',
            track: {
                uri: uri,
                lyrics: null,
                lyrics_url: null
            }
        });

        sendRequest(dispatch, getState, url)
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
                            track: {
                                uri: uri,
                                lyrics: lyrics_html,
                                lyrics_url: url
                            }
                        });
                    }
                },
                error => {
                    dispatch(coreActions.handleException(
                        'Could not extract track lyrics',
                        error
                    ));
                }
            );
    }
}

export function findTrackLyrics(track){
    return (dispatch, getState) => {

        var query = '';
        query += track.artists[0].name+' ';
        query += track.name;
        query = query.toLowerCase();
        query = query.replace(/\([^)]*\) */g, '');        // anything in circle-braces
        query = query.replace(/\([^[]*\] */g, '');        // anything in square-braces
        query = query.replace(/[^A-Za-z0-9\s]/g, '');     // non-alphanumeric

        sendRequest(dispatch, getState, 'search?q='+encodeURIComponent(query))
            .then(
                response => {
                    if (response.response.hits && response.response.hits.length > 0){
                        var lyrics_results = [];
                        for (var i = 0; i < response.response.hits.length; i++){
                            lyrics_results.push({
                                title: response.response.hits[i].result.full_title,
                                url: response.response.hits[i].result.url
                            });
                        }
                        dispatch({
                            type: 'TRACK_LOADED',
                            track: {
                                uri: track.uri,
                                lyrics_results: lyrics_results
                            }
                        });
                        dispatch(getTrackLyrics(track.uri, lyrics_results[0].url));
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

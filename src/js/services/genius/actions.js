
var coreActions = require('../core/actions')
var uiActions = require('../ui/actions')
var helpers = require('../../helpers')

/**
 * Send an ajax request
 *
 * @param dispatch obj
 * @param getState obj
 * @param endpoint = the action
 * @param params = string for any URL prarams to pass on
 **/
const sendRequest = (dispatch, getState, endpoint) => {
    return new Promise((resolve, reject) => {

        var loader_key = helpers.generateGuid();
        dispatch(uiActions.startLoading(loader_key, 'genius_'+endpoint));

        var config = {
            method: 'GET',
            url: getState().genius.provider_url+endpoint,
            timeout: 10000
        };

        $.ajax(config).then(
            response => {
                dispatch(uiActions.stopLoading(loader_key));
                resolve(response);
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

        sendRequest(dispatch, getState, "?action=lyrics&url="+url)
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

        sendRequest(dispatch, getState, '?action=search&query='+encodeURIComponent(query))
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

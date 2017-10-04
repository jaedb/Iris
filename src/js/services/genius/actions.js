
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
const sendRequest = (dispatch, getState, endpoint) => {
    return new Promise( (resolve, reject) => {

        var loader_key = helpers.generateGuid();
        dispatch(uiActions.startLoading(loader_key, 'genius_'+endpoint));

        var config = {
            method: 'GET',
            cache: false,
            url: 'https://api.genius.com/'+endpoint+'&access_token=2AGP9sfzKQcxfKSZuGa_3lqsDIpuOiTGT7-vhJYcKaaDjHIIA2HICsxXCiC30Xxi'
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

export function getTrackInfo(track){
    return (dispatch, getState) => {

        var query = '';
        for (var i = 0; i < track.artists.length; i++){
            query += track.artists[i].name+' ';
        }
        query += track.name;

        sendRequest(dispatch, getState, 'search?q='+query)
            .then(
                response => {
                    if (response.hits && response.hits.length > 0){
                        dispatch({
                            type: 'TRACK_LOADED',
                            key: track.uri,
                            track: {
                                annotations: response.hits[0].result
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
            )
    }
}

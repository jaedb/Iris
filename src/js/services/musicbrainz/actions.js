
var coreActions = require('../core/actions');
var uiActions = require('../ui/actions');
var helpers = require('../../helpers');

/**
 * Send an ajax request to the LastFM API
 *
 * @param dispatch = obj
 * @param endpoint = String
 * @param params = String
 **/
var sendRequest = (dispatch, endpoint, params) => {
    return new Promise((resolve, reject) => {
        const loader_key = helpers.generateGuid();

        dispatch(uiActions.startLoading(loader_key, ',musicbrainz_'+endpoint));

        var config = {
            method: 'GET',
            cache: true,
            timeout: 30000,
            url: 'https://musicbrainz.org/ws/2/'+endpoint+'?fmt=json&'+params
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
                    if (xhr && xhr.responseJSON && xhr.responseJSON.message){
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

export function findArtist(uri, artist){
    return (dispatch, getState) => {
        sendRequest(dispatch, 'artist', 'query=artist:'+artist.name)
            .then(
                response => {
                    console.log(response);
                },
                error => {
                    console.info("Musicbrainz: No results for artist '"+artist.name+"'");
                }
            )
    }
}

export function getArtist(uri, artist){

    return (dispatch, getState) => {
        sendRequest(dispatch, 'artist/'+artist.mbid, 'inc=url-rels')
            .then(
                response => {
                    if (response){
                        const image_relations = response.relations.filter(rel => (rel['target-type'] === 'image'));
                        const images = image_relations.map(ir => ir.url.resource);
                    	var artist = {
                            uri: uri,
                            images: images,
                        };
                        console.log(response);
                        console.log(image_relations);
                        dispatch(coreActions.artistLoaded(artist));
                    }
                },
                error => {
                    console.info("Musicbrainz: No results for artist '"+artist.uri+"'");
                }
            )
    }
}

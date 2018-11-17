
import ReactGA from 'react-ga';
import Mopidy from 'mopidy';
import md5 from 'md5';
import { hashHistory } from 'react-router';
import * as helpers from '../../helpers';

var mopidyActions = require('./actions.js');
var coreActions = require('../core/actions.js');
var uiActions = require('../ui/actions.js');
var spotifyActions = require('../spotify/actions.js');
var pusherActions = require('../pusher/actions.js');
var googleActions = require('../google/actions.js');
var lastfmActions = require('../lastfm/actions.js');

const MopidyMiddleware = (function(){

    // container for the actual Mopidy socket
    var socket = null;

    // play position timer
    var progress_interval = null;
    var progress_interval_counter = 0;

    // handle all manner of socket messages
    const handleMessage = (ws, store, type, data) => {

        // if debug enabled
        if (store.getState().ui.log_mopidy){
            console.log('Mopidy', type, data);
        }

        switch(type){

            case 'state:online':
                store.dispatch({ type: 'MOPIDY_CONNECTED' });

                store.dispatch(mopidyActions.getPlayState());
                store.dispatch(mopidyActions.getVolume());
                store.dispatch(mopidyActions.getMute());
                store.dispatch(mopidyActions.getConsume());
                store.dispatch(mopidyActions.getRandom());
                store.dispatch(mopidyActions.getRepeat());
                store.dispatch(mopidyActions.getQueue());
                store.dispatch(mopidyActions.getCurrentTrack());
                store.dispatch(mopidyActions.getTimePosition());
                store.dispatch(mopidyActions.getUriSchemes());

                // every 1000s update our play position (when playing)
                progress_interval = setInterval(() => {
                    if (store.getState().mopidy.play_state == 'playing'){

                        // every 10s get real position from server
                        if (progress_interval_counter % 5 == 0){
                            store.dispatch(mopidyActions.getTimePosition());

                        // otherwise we just assume to add 1000ms every 1000ms of play time
                        } else {
                            store.dispatch(mopidyActions.timePosition(store.getState().mopidy.time_position + 1000));
                        }

                        progress_interval_counter++
                    }
                }, 1000);

                break;

            case 'state:offline':
                store.dispatch({ type: 'MOPIDY_DISCONNECTED' });
                store.dispatch(mopidyActions.clearCurrentTrack());

                // reset our playback interval timer
                clearInterval(progress_interval)
                progress_interval_counter = 0
                break;

            case 'event:tracklistChanged':
                store.dispatch(mopidyActions.getQueue());
                store.dispatch(mopidyActions.getNextTrack());
                break;

            case 'event:playbackStateChanged':
                store.dispatch({
                    type: 'MOPIDY_PLAY_STATE',
                    play_state: data.new_state
                });
                store.dispatch(mopidyActions.getTimePosition());
                break;

            case 'event:seeked':
                store.dispatch({
                    type: 'MOPIDY_TIME_POSITION',
                    time_position: data.time_position
                });
                break;

            case 'event:trackPlaybackEnded':
                store.dispatch(mopidyActions.clearCurrentTrack());
                store.dispatch({
                    type: 'MOPIDY_TIME_POSITION',
                    time_position: 0
                });
                break;

            case 'event:trackPlaybackStarted':
                store.dispatch(mopidyActions.currentTrackLoaded(data.tl_track));
                store.dispatch(mopidyActions.getNextTrack());
                break;

            case 'event:volumeChanged':
                store.dispatch({type: 'MOPIDY_VOLUME', volume: data.volume});
                break;

            case 'event:muteChanged':
                store.dispatch({type: 'MOPIDY_MUTE', mute: data.mute});
                break;

            case 'event:optionsChanged':
                store.dispatch(mopidyActions.getConsume());
                store.dispatch(mopidyActions.getRandom());
                store.dispatch(mopidyActions.getRepeat());
                break;

            default:
                //console.log('MopidyService: Unhandled message', type, message );
        }
    }


    /**
     * Call something with Mopidy
     *
     * Sends request to Mopidy server, and updates our local storage on return
     * @param object ws = Mopidy class that wraps a Websocket
     * @param string model = which Mopidy model (playback, tracklist, etc)
     * @param string property = TlTracks, Consume, etc
     * @param string value (optional) = value of the property to pass
     * @return promise
     **/
    const request = (ws, store, call, value = {}) => {
        if (!store.getState().mopidy.connected){
            return false;
        }

        var callParts = call.split('.');
        var model = callParts[0];
        var method = callParts[1];

        return new Promise((resolve, reject) => {
            if (method in ws[model]){
                var mopidyObject = ws[model][method];
                var property = method;
            } else {
                var mopidyObject = ws[model];
                var property = model;
            }

            // Detect invalid model.method calls, which result in an empty mopidyObject
            if (!mopidyObject || typeof(mopidyObject) !== 'function'){
                var error = {
                    message: 'Call to an invalid object. Check you are calling a valid Mopidy object.',
                    call: call,
                    value: value
                }

                store.dispatch(coreActions.handleException(
                    'Mopidy: '+error.message,
                    error
                ));

                reject(error);
            }

            var loader_key = helpers.generateGuid();
            store.dispatch(uiActions.startLoading(loader_key, 'mopidy_'+property));

            // Start our 15 second timeout
            var timeout = setTimeout(
                function(){
                    store.dispatch(uiActions.stopLoading(loader_key));
                    reject({message: "Request timed out", call: call, value: value});
                },
                30000
            );

            mopidyObject(value)
                .then(
                    response => {
                        clearTimeout(timeout);
                        store.dispatch(uiActions.stopLoading(loader_key));
                        resolve(response);
                    },
                    error => {
                        clearTimeout(timeout);
                        store.dispatch(uiActions.stopLoading(loader_key));
                        reject(error);
                    }
                );
        });
    }


    /**
     * Middleware
     *
     * This behaves like an action interceptor. We listen for specific actions
     * and handle special functionality. If the action is not in our switch, then
     * it just proceeds to the next middleware, or default functionality
     **/
    return store => next => action => {
        switch(action.type){

            case 'MOPIDY_CONNECT':
                if (socket != null){
                	socket.close();
                }

                store.dispatch({ type: 'MOPIDY_CONNECTING'});
                var state = store.getState();

                socket = new Mopidy({
                    webSocketUrl: 'ws'+(window.location.protocol === 'https:' ? 's' : '')+'://'+state.mopidy.host+':'+state.mopidy.port+'/mopidy/ws/',
                    callingConvention: 'by-position-or-by-name'
                });

                socket.on((type, data) => handleMessage(socket, store, type, data));
                break;

            case 'MOPIDY_CONNECTED':
                if (store.getState().ui.allow_reporting){
                    var hashed_hostname = md5(window.location.hostname);
	                ReactGA.event({ category: 'Mopidy', action: 'Connected', label: hashed_hostname });
	            }
                store.dispatch(uiActions.createNotification({content: 'Mopidy connected'}));
                next(action);
                break;

            case 'MOPIDY_DISCONNECT':
                if (socket != null) socket.close()
                socket = null
                store.dispatch({ type: 'MOPIDY_DISCONNECTED' })
                break;

            case 'MOPIDY_DISCONNECTED':
                store.dispatch(uiActions.createNotification({type: 'bad', content: 'Mopidy disconnected'}));
                break;

            case 'MOPIDY_DEBUG':
                request(socket, store, action.call, action.value )
                    .then(response => {
                        store.dispatch({type: 'DEBUG', response: response});
                    })
                break;

            case 'MOPIDY_REQUEST':
                request(socket, store, action.method, action.params)
                    .then(
                        response => {
	                        if (action.response_callback){
	                            store.dispatch(action.response_callback.call(this, response));
	                        }
                        },
                        error => {
	                        if (action.error_callback){
	                            store.dispatch(action.error_callback.call(this, error));
	                        } else {
	                            store.dispatch(coreActions.handleException(
	                                'Mopidy request failed',
	                                error,
	                                action.method,
	                                action
	                            ));
	                        }
                        }
                    );
                break

            /**
             * General playback
             **/

            case 'MOPIDY_PLAY_STATE':
                store.dispatch(uiActions.setWindowTitle(null, action.play_state));
                next(action);
                break

            case 'MOPIDY_GET_PLAY_STATE':
                request(socket, store, 'playback.getState')
                    .then(
                        response => {
                            store.dispatch({
                                type: 'MOPIDY_PLAY_STATE',
                                play_state: response
                            })
                        }
                    )
                break

            case 'MOPIDY_PLAY':
                request(socket, store, 'playback.play')
                    .then(
                        response => {
                            store.dispatch({
                                type: 'MOPIDY_PLAY_STATE',
                                play_state: 'playing'
                            })
                        }
                    );

                store.dispatch(pusherActions.deliverBroadcast(
                    'notification',
                    {
                        notification: {
                            content: store.getState().pusher.username +(store.getState().mopidy.play_state == 'paused' ? ' resumed' : ' started')+' playback',
                            icon: (store.getState().core.current_track ? helpers.getTrackIcon(store.getState().core.current_track, store.getState().core) : false)
                        }
                    }
                ));
            break;

            case 'MOPIDY_PAUSE':
                request(socket, store, 'playback.pause')
                    .then(
                        response => {
                            store.dispatch({
                                type: 'MOPIDY_PLAY_STATE',
                                play_state: 'paused'
                            })
                        }
                    );

                store.dispatch(pusherActions.deliverBroadcast(
                    'notification',
                    {
                        notification: {
                            content: store.getState().pusher.username +' paused playback',
                            icon: (store.getState().core.current_track ? helpers.getTrackIcon(store.getState().core.current_track, store.getState().core) : false)
                        }
                    }
                ));
                break

            case 'MOPIDY_PREVIOUS':
                request(socket, store, 'playback.previous');
                break

            case 'MOPIDY_NEXT':
                request(socket, store, 'playback.next');

                store.dispatch(pusherActions.deliverBroadcast(
                    'notification',
                    {
                        notification: {
                            content: store.getState().pusher.username +' skipped <em>'+store.getState().core.current_track.name+'</em>',
                            icon: (store.getState().core.current_track ? helpers.getTrackIcon(store.getState().core.current_track, store.getState().core) : false)
                        }
                    }
                ));
                break

            case 'MOPIDY_STOP':
                request(socket, store, 'playback.stop')
                    .then(response => {
                            store.dispatch(mopidyActions.clearCurrentTrack());
                        });

                store.dispatch(pusherActions.deliverBroadcast(
                    'notification',
                    {
                        notification: {
                            content: store.getState().pusher.username +' stopped playback',
                            icon: (store.getState().core.current_track ? helpers.getTrackIcon(store.getState().core.current_track, store.getState().core) : false)
                        }
                    }
                ));
                break

            case 'MOPIDY_CHANGE_TRACK':
                request(socket, store, 'playback.play', {tlid: action.tlid});

                store.dispatch(pusherActions.deliverBroadcast(
                    'notification',
                    {
                        notification: {
                            content: store.getState().pusher.username +' changed track'
                        }
                    }
                ));
                break;

            case 'MOPIDY_REMOVE_TRACKS':
                request(socket, store, 'tracklist.remove', {tlid: action.tlids});
                store.dispatch(pusherActions.deliverBroadcast(
                    'notification',
                    {
                        notification: {
                            content: store.getState().pusher.username +' removed '+action.tlids.length+' tracks'
                        }
                    }
                ));
                break;

            case 'MOPIDY_GET_REPEAT':
                request(socket, store, 'tracklist.getRepeat')
                    .then(
                        response => {
                            store.dispatch({
                                type: 'MOPIDY_REPEAT',
                                repeat: response
                            });
                        }
                    );
                break;

            case 'MOPIDY_SET_REPEAT':
                request(socket, store, 'tracklist.setRepeat', [action.repeat]);
                break;

            case 'MOPIDY_GET_RANDOM':
                request(socket, store, 'tracklist.getRandom')
                    .then(
                        response => {
                            store.dispatch({
                                type: 'MOPIDY_RANDOM',
                                random: response
                            });
                        }
                    );
                break;

            case 'MOPIDY_SET_RANDOM':
                request(socket, store, 'tracklist.setRandom', [action.random]);
                break;

            case 'MOPIDY_GET_CONSUME':
                request(socket, store, 'tracklist.getConsume')
                    .then(
                        response => {
                            store.dispatch({
                                type: 'MOPIDY_CONSUME',
                                consume: response
                            });
                        }
                    );
                break;

            case 'MOPIDY_SET_CONSUME':
                request(socket, store, 'tracklist.setConsume', [action.consume]);
                break;

            case 'MOPIDY_GET_MUTE':
                request(socket, store, 'mixer.getMute')
                    .then(
                        response => {
                            store.dispatch({
                                type: 'MOPIDY_MUTE',
                                mute: response
                            });
                        }
                    );
                break;

            case 'MOPIDY_SET_MUTE':
                request(socket, store, 'mixer.setMute', [action.mute]);
                store.dispatch(pusherActions.deliverBroadcast(
                    'notification',
                    {
                        notification: {
                            content: store.getState().pusher.username +(action.mute ? ' muted' : ' unmuted')+' playback'
                        }
                    }
                ));
                break;

            case 'MOPIDY_GET_VOLUME':
                request(socket, store, 'playback.getVolume')
                    .then(
                        response => {
                            store.dispatch({
                                type: 'MOPIDY_VOLUME',
                                volume: response
                            });
                        }
                    );
                break;

            case 'MOPIDY_SET_VOLUME':
                request(socket, store, 'playback.setVolume', {volume: action.volume})
                    .then(
                        response => {
                            store.dispatch({
                                type: 'MOPIDY_VOLUME',
                                volume: action.volume
                            });
                        }
                    );
                break;

            case 'MOPIDY_SET_TIME_POSITION':
                request(socket, store, 'playback.seek', {time_position: action.time_position})
                    .then(
                        response => {
                            store.dispatch({
                                type: 'MOPIDY_TIME_POSITION',
                                time_position: action.time_position
                            });
                        }
                    );
                break;

            case 'MOPIDY_GET_TIME_POSITION':
                request(socket, store, 'playback.getTimePosition')
                    .then(
                        response => {
                            store.dispatch({
                                type: 'MOPIDY_TIME_POSITION',
                                time_position: response
                            });
                        }
                    );
                break;

            case 'MOPIDY_GET_URI_SCHEMES':
                request(socket, store, 'getUriSchemes')
                    .then(
                        response => {
                            var uri_schemes = response;
                            var remove = ['http','https','mms','rtmp','rtmps','rtsp','sc','yt'];

                            // remove all our ignored types
                            for(var i = 0; i < remove.length; i++){
                                var index = uri_schemes.indexOf(remove[i]);
                                if (index > -1 ) uri_schemes.splice(index, 1);
                            }

                            // append with ':' to make them a mopidy URI
                            for(var i = 0; i < uri_schemes.length; i++){
                                uri_schemes[i] = uri_schemes[i] +':';
                            }

                            // Enable Iris providers when the backend is available
                            store.dispatch(spotifyActions.set({enabled: uri_schemes.includes('spotify:')}));
                            store.dispatch(googleActions.set({enabled: uri_schemes.includes('gmusic:')}));

                            // If we haven't customised our search schemes, add all to search
                            if (store.getState().ui.uri_schemes_search_enabled === undefined){
                                store.dispatch(uiActions.set({uri_schemes_search_enabled: uri_schemes}));
                            }

                            store.dispatch({type: 'MOPIDY_URI_SCHEMES', uri_schemes: uri_schemes});
                        }
                    );
                break;


            /**
             * Advanced playback events
             **/

            case 'MOPIDY_PLAY_PLAYLIST':

                // Clear tracklist (if set)
                if (store.getState().ui.clear_tracklist_on_play){
                    store.dispatch(mopidyActions.clearTracklist())
                }

                // playlist already in index
                if (store.getState().core.playlists.hasOwnProperty(action.uri)){

                    // Spotify-provied playlists need to be handled by the Spotify service
                    if (store.getState().core.playlists[action.uri].provider == 'spotify'){
                        store.dispatch(spotifyActions.getPlaylistTracksAndPlay(action.uri, action.shuffle))
                        break
                    }

                // it's a spotify playlist that we haven't loaded
                // we need to fetch via HTTP API to avoid timeout
                } else if (helpers.uriSource(action.uri) == 'spotify' && store.getState().spotify.enabled){
                    store.dispatch(spotifyActions.getPlaylistTracksAndPlay(action.uri, action.shuffle))
                    break

                // Not in index, and Spotify HTTP not enabled, so just play it as-is
                }

                // fetch the playlist tracks via backend
                // add each track by URI
                request(socket, store, 'playlists.lookup', {uri: action.uri})
                    .then(
                        response => {
                            if (response.tracks === undefined){
                                store.dispatch(uiActions.createNotification({content: 'Failed to load playlist tracks', type: 'bad'}));
                            } else {
                                var tracks_uris = helpers.arrayOf('uri',response.tracks);
                                if (action.shuffle){
                                	tracks_uris = helpers.shuffle(tracks_uris);
                                }
                                store.dispatch(mopidyActions.playURIs(tracks_uris, action.uri))
                            }
                        },
                        error => {
                            store.dispatch(coreActions.handleException(
                                "Mopidy: "+(error.message ? error.message : "Lookup failed"),
                                error
                            ));
                        }
                    );
                break;

            case 'MOPIDY_ENQUEUE_URIS':

                if (!action.uris || action.uris.length <= 0){
                    this.props.uiActions.createNotification({content: "No URIs to enqueue", type: "warning"});
                    break;
                }

                store.dispatch(pusherActions.deliverBroadcast(
                    'notification',
                    {
                        notification: {
                            content: store.getState().pusher.username +' is adding '+action.uris.length+' URIs to queue',
                            icon: (store.getState().core.current_track ? helpers.getTrackIcon(store.getState().core.current_track, store.getState().core) : false)
                        }
                    }
                ));

                // split into batches
                var uris = Object.assign([], action.uris)
                var batches = []
                var batch_size = 5
                while (uris.length > 0){
                    batches.push({
                        uris: uris.splice(0,batch_size),
                        at_position: action.at_position,
                        next: action.next,
                        offset: action.offset + (batch_size * batches.length),
                        from_uri: action.from_uri
                    })
                }

                // pass this modified action to the reducer (and other middleware)
                action.batches = batches
                next(action)

                // start our processor
                store.dispatch(uiActions.startProcess(
                    'MOPIDY_ENQUEUE_URIS_PROCESSOR',
                    'Adding '+action.uris.length+' URI(s)',
                    {
                        batches: batches,
                        remaining: action.uris.length,
                        total: action.uris.length
                    }
                ))
                break

            case 'MOPIDY_ENQUEUE_URIS_PROCESSOR':

                var last_run = store.getState().ui.processes.MOPIDY_ENQUEUE_URIS_PROCESSOR

                // Cancelling
                if (last_run && last_run.status == 'cancelling'){
                    store.dispatch(uiActions.processCancelled('MOPIDY_ENQUEUE_URIS_PROCESSOR'))
                    return

                // make sure we have some uris in the queue
                } else if (action.data.batches && action.data.batches.length > 0){

                    var batches = Object.assign([],action.data.batches)
                    var batch = batches[0]
                    var total_uris = 0
                    for (var i = 0; i < batches.length; i++){
                        total_uris += batches[i].uris.length
                    }
                    batches.shift()
                    store.dispatch(uiActions.updateProcess(
                        'MOPIDY_ENQUEUE_URIS_PROCESSOR',
                        'Adding '+total_uris+' URI(s)',
                        {
                            remaining: total_uris
                        }
                    ))

                // no batches means we're done here
                } else {
                    store.dispatch(uiActions.processFinishing('MOPIDY_ENQUEUE_URIS_PROCESSOR'))
                    break
                }

                var current_track = store.getState().core.current_track
                var queue = store.getState().core.queue
                var current_track_index = -1

                if (current_track){
                    for (var i = 0; i < queue.length; i++){
                        if (queue[i].tlid == current_track.tlid){
                            current_track_index = i
                            break
                        }
                    }
                }

                var params = {uris: batch.uris}

                // Play this batch next
                if (batch.next){

                    // Make sure we're playing something first
                    if (current_track_index > -1){
                        params.at_position = current_track_index + batch.offset + 1

                    // Default to top of queue if we're not playing
                    } else {
                        params.at_position = 0 + batch.offset
                    }

                // A specific position has been defined
                // NOTE: This is likely to be wrong as the original action is unaware of batches or other client requests
                } else if (batch.at_position){
                    params.at_position = batch.at_position + batch.offset
                }

                request(socket, store, 'tracklist.add', params)
                    .then(
                        response => {
                            // add metadata to queue
                            var tlids = []
                            for (var i = 0; i < response.length; i++){
                                tlids.push(response[i].tlid)
                            }
                            store.dispatch(pusherActions.addQueueMetadata(tlids, batch.from_uri))

                            // Re-run the batch checker in 100ms. This allows a small window for other
                            // server requests before our next batch. It's a little crude but it means the server isn't
                            // locked until we're completely done.
                            setTimeout(
                                function(){
                                    store.dispatch(uiActions.runProcess(action.type, {batches: batches}))
                                },
                                100
                            )
                        },
                        error => {
                            store.dispatch(coreActions.handleException(
                                "Mopidy: "+(error.message ? error.message : "Adding tracks failed"),
                                error
                            ));
                        }
                    )

                break

            case 'MOPIDY_PLAY_URIS':

                if (!action.uris || action.uris.length <= 0){
                    this.props.uiActions.createNotification({content: "No URIs to play", type: "warning"});
                    break;
                }

                // Stop the radio
                if (store.getState().core.radio && store.getState().core.radio.enabled){
                    store.dispatch(pusherActions.stopRadio());
                }

                // Clear tracklist (if set)
                if (store.getState().ui.clear_tracklist_on_play){
                    store.dispatch(mopidyActions.clearTracklist());
                }

                // Shuffle/random mode
                if (store.getState().mopidy.random){
                	var first_uri_index = Math.floor(Math.random() * action.uris.length);
	            } else {
                	var first_uri_index = 0;
	            }
	            var first_uri = action.uris[first_uri_index];

                // add our first track
                request(socket, store, 'tracklist.add', { uri: first_uri, at_position: 0 })
                    .then(
                        response => {

                            // play it (only if we got a successful lookup)
                            if (response.length > 0){
                                store.dispatch(mopidyActions.changeTrack(response[0].tlid));

                                var tlids = []
                                for (var i = 0; i < response.length; i++){
                                    tlids.push(response[i].tlid)
                                }
                                store.dispatch(pusherActions.addQueueMetadata(tlids, action.from_uri))
                            } else {
                                store.dispatch(coreActions.handleException(
                                    "Mopidy: Failed to add some tracks",
                                    response
                                ));
                            }

                            // Remove our first_uri as we've already added it
                            action.uris.splice(first_uri_index, 1);

                            // And add the rest of our uris (if any)
                            if (action.uris.length > 0){

                                // Wait a moment so the server can trigger track_changed etc
                                // this means our UI feels snappier as the first track shows up quickly
                                setTimeout(
                                    function(){
                                        store.dispatch(mopidyActions.enqueueURIs(action.uris, action.from_uri, null, 1))
                                    },
                                    100
                                )
                            }
                        },
                        error => {
                            store.dispatch(coreActions.handleException(
                                "Mopidy: "+(error.message ? error.message : "Adding tracks failed"),
                                error
                            ));
                        }
                    )
                break;

            case 'MOPIDY_REORDER_TRACKLIST':

                // add our first track
                request(socket, store, 'tracklist.move', { start: action.range_start, end: action.range_start + action.range_length, to_position: action.insert_before } )
                    .then(
                        response => {
                            // TODO: when complete, send event to confirm success/failure
                        },
                        error => {
                            store.dispatch(coreActions.handleException(
                                "Mopidy: "+(error.message ? error.message : "Reorder failed"),
                                error
                            ));
                        }
                    )
                break;

            case 'MOPIDY_CLEAR_TRACKLIST':
                request(socket, store, 'tracklist.clear')
                	.then(
                		response => {
                			store.dispatch(coreActions.clearCurrentTrack());

			                store.dispatch(pusherActions.deliverBroadcast(
			                    'notification',
			                    {
			                        notification: {
			                            content: store.getState().pusher.username +' cleared queue'
			                        }
			                    }
			                ));
                		}
                	);
                break;


            /**
             * =============================================================== SEARCHING ============
             * ======================================================================================
             **/


            case 'MOPIDY_GET_SEARCH_RESULTS':

                // Flush out our previous results
                store.dispatch({type: 'MOPIDY_CLEAR_SEARCH_RESULTS'});

                var uri_schemes_to_ignore = ['spotify:'];
                var uri_schemes = Object.assign([], store.getState().ui.uri_schemes_search_enabled);
                for (var i = 0; i < uri_schemes.length; i++){
                    if (uri_schemes_to_ignore.includes(uri_schemes[i])){
                        uri_schemes.splice(i,1);
                    }
                }
                var uri_schemes_total = uri_schemes.length;
                var uri_scheme = uri_schemes.shift();

                if (uri_schemes_total <= 0){
                    store.dispatch(uiActions.createNotification({content: 'No sources selected', type: 'warning'}));
                } else {
                    store.dispatch(uiActions.startProcess(
                        'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                        'Searching '+uri_schemes_total+' Mopidy providers',
                        {
                            context: action.context,
                            query: action.query,
                            limit: action.limit,
                            total: uri_schemes_total,
                            remaining: uri_schemes.length,
                            uri_scheme: uri_scheme,
                            uri_schemes: uri_schemes
                        }
                    ));
                }

                break


            case 'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR':
                var last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;

                // Cancelling
                if (last_run && last_run.status == 'cancelling'){
                    store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
                    return

                // No more schemes, so we're done!
                } else if (!action.data.uri_scheme){
                    store.dispatch(uiActions.processFinishing('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'))
                    return
                }

                // Construct our next batch's task
                var next_uri_schemes = Object.assign([], action.data.uri_schemes)
                var next_uri_scheme = next_uri_schemes.shift()

                // Update UI for this round
                store.dispatch(uiActions.updateProcess(
                    'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                    'Searching '+action.data.uri_scheme.replace(':',''),
                    {
                        remaining: action.data.uri_schemes.length
                    }
                ))

                switch (action.data.context){

                    // Albums
                    case 'albums':
                            
                        // Quick check to see if we should be cancelling
                        var last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;
                        if (last_run && last_run.status == 'cancelling'){
                            store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
                            return;
                        }

                        store.dispatch(uiActions.updateProcess(
                            'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                            'Searching '+action.data.uri_scheme.replace(':','')
                        ))

                        var continue_process = () => {
                            store.dispatch(uiActions.runProcess(
                                'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                                {
                                    context: action.data.context,
                                    query: action.data.query,
                                    limit: action.data.limit,
                                    uri_scheme: next_uri_scheme,
                                    uri_schemes: next_uri_schemes
                                }
                            ))
                        }

                        request(socket, store, 'library.search', {query: {album: [action.data.query]}, uris: [action.data.uri_scheme]})
                            .then(
                                response => {
                                    if (response.length > 0){
                                        var albums = [];

                                        // Merge our proper album response container
                                        if (response[0].albums){
                                            albums = [...response[0].albums, ...albums];
                                        }

                                        // Pull the Album objects from our track responses
                                        if (response[0].tracks){
                                            for (var i = 0; i < response[0].tracks.length; i++){
                                                if (response[0].tracks[i].album !== undefined && response[0].tracks[i].album.uri !== undefined){
                                                    albums.push(response[0].tracks[i].album)
                                                }
                                            }
                                        }

                                        var albums_uris = helpers.arrayOf('uri',albums)
                                        albums_uris = helpers.removeDuplicates(albums_uris)

                                        store.dispatch(coreActions.albumsLoaded(albums));

                                        // and plug in their URIs
                                        store.dispatch({
                                            type: 'MOPIDY_SEARCH_RESULTS_LOADED',
                                            context: action.data.context,
                                            results: albums_uris
                                        })
                                    }

                                    continue_process();
                                },
                                error => {
                                    store.dispatch(coreActions.handleException(
                                        "Mopidy: "+(error.message ? error.message : "Search failed"),
                                        error
                                    ));
                                    continue_process();
                                }
                            )
                        break

                    // Artists
                    case 'artists':
                            
                        // Quick check to see if we should be cancelling
                        var last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;
                        if (last_run && last_run.status == 'cancelling'){
                            store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
                            return;
                        }

                        store.dispatch(uiActions.updateProcess(
                            'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                            'Searching '+action.data.uri_scheme.replace(':','')
                        ))

                        var continue_process = () => {
                            store.dispatch(uiActions.runProcess(
                                'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                                {
                                    context: action.data.context,
                                    query: action.data.query,
                                    limit: action.data.limit,
                                    uri_scheme: next_uri_scheme,
                                    uri_schemes: next_uri_schemes
                                }
                            ))
                        }

                        request(socket, store, 'library.search', {query: {artist: [action.data.query]}, uris: [action.data.uri_scheme]})
                            .then(
                                response => {
                                    if (response.length > 0){
                                        var artists_uris = [];

                                        // Pull actual artist objects
                                        if (response[0].artists){
                                            for (var i = 0; i < response[0].artists.length; i++){
                                                artists_uris.push(response[0].artists.uri);
                                            }
                                        }

                                        // Digest track artists into actual artist results
                                        if (response[0].tracks){
                                            for (var i = 0; i < response[0].tracks.length; i++){
                                                if (response[0].tracks[i].artists){
                                                    for (var j = 0; j < response[0].tracks[i].artists.length; j++){
                                                        var artist = response[0].tracks[i].artists[j]
                                                        if (artist.uri){
                                                            artists_uris.push(artist.uri)
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        artists_uris = helpers.removeDuplicates(artists_uris);

                                        // load each artist
                                        for (var i = 0; i < artists_uris.length; i++){
                                            store.dispatch(mopidyActions.getArtist(artists_uris[i]));
                                        }

                                        // and plug in their URIs
                                        store.dispatch({
                                            type: 'MOPIDY_SEARCH_RESULTS_LOADED',
                                            context: action.data.context,
                                            results: artists_uris
                                        })
                                    }

                                    continue_process();
                                },
                                error => {
                                    store.dispatch(coreActions.handleException(
                                        "Mopidy: "+(error.message ? error.message : "Search failed"),
                                        error
                                    ));
                                    continue_process();
                                }
                            );
                        break;

                    // Playlists
                    case 'playlists':
                            
                        // Quick check to see if we should be cancelling
                        var last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;
                        if (last_run && last_run.status == 'cancelling'){
                            store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
                            return;
                        }

                        store.dispatch(uiActions.updateProcess(
                            'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                            'Searching playlists'
                        ))

                        var continue_process = () => {
                            store.dispatch(uiActions.processFinishing('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'))
                        }

                        request(socket, store, 'playlists.asList')
                            .then(
                                response => {
                                    if (response.length > 0){
                                        var playlists_uris = [];

                                        for (var i = 0; i < response.length; i++){
                                            var playlist = response[i]
                                            if (playlist.name.includes(action.data.query) && action.data.uri_schemes.includes(helpers.uriSource(playlist.uri)+':')){
                                                playlists_uris.push(playlist.uri)
                                            }
                                        }

                                        // load each playlist
                                        for (var i = 0; i < playlists_uris.length; i++){
                                            store.dispatch(mopidyActions.getPlaylist(playlists_uris[i]))
                                        }

                                        // and plug in their URIs
                                        store.dispatch({
                                            type: 'MOPIDY_SEARCH_RESULTS_LOADED',
                                            context: action.data.context,
                                            results: playlists_uris
                                        })
                                    }
                                    continue_process();
                                },
                                error => {
                                    store.dispatch(coreActions.handleException(
                                        "Mopidy: "+(error.message ? error.message : "Search failed"),
                                        error
                                    ));
                                    continue_process();
                                }
                            )
                        break

                    // Tracks
                    case 'tracks':
                            
                        // Quick check to see if we should be cancelling
                        var last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;
                        if (last_run && last_run.status == 'cancelling'){
                            store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
                            return;
                        }

                        store.dispatch(uiActions.updateProcess(
                            'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                            'Searching '+action.data.uri_scheme.replace(':','')
                        ));

                        var continue_process = () => {
                            store.dispatch(uiActions.runProcess(
                                'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                                {
                                    context: action.data.context,
                                    query: action.data.query,
                                    limit: action.data.limit,
                                    uri_scheme: next_uri_scheme,
                                    uri_schemes: next_uri_schemes
                                }
                            ))
                        }

                        request(socket, store, 'library.search', {query: {any: [action.data.query]}, uris: [action.data.uri_scheme]})
                            .then(
                                response => {
                                    if (response.length > 0 && response[0].tracks !== undefined){
                                        var tracks = response[0].tracks;

                                        store.dispatch({
                                            type: 'MOPIDY_SEARCH_RESULTS_LOADED',
                                            context: action.data.context,
                                            results: helpers.formatTracks(tracks)
                                        });
                                    }
                                    continue_process();
                                },
                                error => {
                                    store.dispatch(coreActions.handleException(
                                        "Mopidy: "+(error.message ? error.message : "Search failed"),
                                        error
                                    ));
                                    continue_process();
                                }
                            )

                        break

                    // Search for all types
                    case 'all':
                    default:

                        var process_tracks = () => {
                            
                            // Quick check to see if we should be cancelling
                            var last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;
                            if (last_run && last_run.status == 'cancelling'){
                                store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
                                return;
                            }

                            store.dispatch(uiActions.updateProcess(
                                'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                                'Searching '+action.data.uri_scheme.replace(':','')+' tracks',
                                {
                                    remaining: (action.data.uri_schemes.length) + 1
                                }
                            ));
                            request(socket, store, 'library.search', {query: {any: [action.data.query]}, uris: [action.data.uri_scheme]})
                                .then(
                                    response => {
                                        if (response.length > 0 && response[0].tracks !== undefined){
                                            var tracks = response[0].tracks;

                                            store.dispatch({
                                                type: 'MOPIDY_SEARCH_RESULTS_LOADED',
                                                context: 'tracks',
                                                results: helpers.formatTracks(tracks)
                                            });
                                        }

                                        process_albums();
                                    },
                                    error => {
                                        store.dispatch(coreActions.handleException(
                                            "Mopidy: "+(error.message ? error.message : "Search failed"),
                                            error
                                        ));
                                        process_albums();
                                    }
                                );
                        }

                        var process_albums = () => {
                            
                            // Quick check to see if we should be cancelling
                            var last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;
                            if (last_run && last_run.status == 'cancelling'){
                                store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
                                return;
                            }

                            store.dispatch(uiActions.updateProcess(
                                'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                                'Searching '+action.data.uri_scheme.replace(':','')+' albums',
                                {
                                    remaining: (action.data.uri_schemes.length) + 0.75
                                }
                            ));
                            request(socket, store, 'library.search', {query: {album: [action.data.query]}, uris: [action.data.uri_scheme]})
                                .then(
                                    response => {
                                        if (response.length > 0){
                                            var albums = [];

                                            // Merge actual album responses first
                                            if (response[0].albums){
                                                albums = [...response[0].albums, ...albums];
                                            }

                                            // Then digest tracks albums
                                            if (response[0].tracks){
                                                for (var i = 0; i < response[0].tracks.length; i++){
                                                    if (response[0].tracks[i].album !== undefined && response[0].tracks[i].album.uri !== undefined){
                                                        albums.push(response[0].tracks[i].album)
                                                    }
                                                }
                                            }

                                            var albums_uris = helpers.arrayOf('uri',albums)
                                            albums_uris = helpers.removeDuplicates(albums_uris)

                                            store.dispatch(coreActions.albumsLoaded(albums));

                                            // and plug in their URIs
                                            store.dispatch({
                                                type: 'MOPIDY_SEARCH_RESULTS_LOADED',
                                                context: 'albums',
                                                results: albums_uris
                                            })
                                        }

                                        process_artists();
                                    },
                                    error => {
                                        store.dispatch(coreActions.handleException(
                                            "Mopidy: "+(error.message ? error.message : "Search failed"),
                                            error
                                        ));
                                        process_artists();
                                    }
                                );
                        }

                        var process_artists = () => {
                            
                            // Quick check to see if we should be cancelling
                            var last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;
                            if (last_run && last_run.status == 'cancelling'){
                                store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
                                return;
                            }

                            store.dispatch(uiActions.updateProcess(
                                'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                                'Searching '+action.data.uri_scheme.replace(':','')+' artists',
                                {
                                    remaining: (action.data.uri_schemes.length) + 0.5
                                }
                            ));
                            request(socket, store, 'library.search', {query: {artist: [action.data.query]}, uris: [action.data.uri_scheme]})
                                .then(
                                    response => {
                                        if (response.length > 0){
                                            var artists_uris = [];

                                            // Pull our actual artists objects
                                            if (response[0].artists){
                                                for (var i = 0; i < response[0].artists.length; i++){
                                                    artists_uris.push(response[0].artists.uri);
                                                }
                                            }

                                            // Digest tracks artists
                                            if (response[0].tracks){
                                                for (var i = 0; i < response[0].tracks.length; i++){
                                                    if (response[0].tracks[i].artists){
                                                        for (var j = 0; j < response[0].tracks[i].artists.length; j++){
                                                            var artist = response[0].tracks[i].artists[j]
                                                            if (artist.uri){
                                                                artists_uris.push(artist.uri)
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            artists_uris = helpers.removeDuplicates(artists_uris)

                                            // load each artist
                                            for (var i = 0; i < artists_uris.length; i++){
                                                store.dispatch(mopidyActions.getArtist(artists_uris[i]))
                                            }

                                            // and plug in their URIs
                                            store.dispatch({
                                                type: 'MOPIDY_SEARCH_RESULTS_LOADED',
                                                context: 'artists',
                                                results: artists_uris
                                            })
                                        }

                                        process_playlists();
                                    },
                                    error => {
                                        store.dispatch(coreActions.handleException(
                                            "Mopidy: "+(error.message ? error.message : "Search failed"),
                                            error
                                        ));
                                        process_playlists();
                                    }
                            );
                        }
                        var process_playlists = () => {
                            
                            // Quick check to see if we should be cancelling
                            var last_run = store.getState().ui.processes.MOPIDY_GET_SEARCH_RESULTS_PROCESSOR;
                            if (last_run && last_run.status == 'cancelling'){
                                store.dispatch(uiActions.processCancelled('MOPIDY_GET_SEARCH_RESULTS_PROCESSOR'));
                                return;
                            }

                            if (action.data.uri_scheme == 'm3u:'){
                                store.dispatch(uiActions.updateProcess(
                                    'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                                    'Searching '+action.data.uri_scheme.replace(':','')+' playlists',
                                    {
                                        remaining: (action.data.uri_schemes.length) + 0.25
                                    }
                                ))
                                request(socket, store, 'playlists.asList')
                                    .then(
                                        response => {
                                            if (response.length > 0){

                                                var playlists_uris = []
                                                for (var i = 0; i < response.length; i++){
                                                    var playlist = response[i]
                                                    if (playlist.name.includes(action.data.query) && action.data.uri_schemes.includes(helpers.uriSource(playlist.uri)+':')){
                                                        playlists_uris.push(playlist.uri)
                                                    }
                                                }

                                                playlists_uris = playlists_uris

                                                // load each playlist
                                                for (var i = 0; i < playlists_uris.length; i++){
                                                    store.dispatch(mopidyActions.getPlaylist(playlists_uris[i]))
                                                }

                                                // and plug in their URIs
                                                store.dispatch({
                                                    type: 'MOPIDY_SEARCH_RESULTS_LOADED',
                                                    context: 'playlists',
                                                    results: playlists_uris
                                                })
                                            }

                                            finished();
                                        },
                                        error => {
                                            store.dispatch(coreActions.handleException(
                                                "Mopidy: "+(error.message ? error.message : "Search failed"),
                                                error
                                            ));
                                            finished();
                                        }
                                    );
                            } else {
                                finished();
                            }
                        }

                        var finished = () => {
                            // We're finally done searching for types on this provider
                            // On to the next scheme!
                            store.dispatch(uiActions.runProcess(
                                'MOPIDY_GET_SEARCH_RESULTS_PROCESSOR',
                                {
                                    context: action.data.context,
                                    query: action.data.query,
                                    limit: action.data.limit,
                                    uri_scheme: next_uri_scheme,
                                    uri_schemes: next_uri_schemes,
                                    remaining: action.data.uri_schemes.length
                                }
                            ))
                        }

                        // Kick things off with the tracks
                        process_tracks();
                }

                break


            /**
             * =============================================================== PLAYLIST(S) ==========
             * ======================================================================================
             **/

            case 'MOPIDY_GET_LIBRARY_PLAYLISTS':
                request(socket, store, 'playlists.asList')
                    .then(response => {

                        // drop in our URI list
                        var playlist_uris = helpers.arrayOf('uri',response)
                        var playlist_uris_filtered = []

                        // Remove any Spotify playlists. These will be handled by our Spotify API
                        for (var i = 0; i < playlist_uris.length; i++){
                            if (helpers.uriSource(playlist_uris[i]) != 'spotify'){
                                playlist_uris_filtered.push(playlist_uris[i])
                            }
                        }

                        store.dispatch({ type: 'MOPIDY_LIBRARY_PLAYLISTS_LOADED', uris: playlist_uris_filtered });
                        store.dispatch({ type: 'MOPIDY_LIBRARY_PLAYLISTS_LOADED_ALL' });

                        // get the full playlist objects
                        for (var i = 0; i < playlist_uris_filtered.length; i++){
                            request(socket, store, 'playlists.lookup', { uri: playlist_uris_filtered[i] })
                                .then(response => {
                                    var source = helpers.uriSource(response.uri)
                                    var playlist = Object.assign(
                                        {},
                                        {
                                            type: 'playlist',
                                            name: response.name,
                                            uri: response.uri,
                                            source: source,
                                            is_mopidy: true,
                                            last_modified: response.last_modified,
                                            tracks_total: (response.tracks ? response.tracks.length : 0 )
                                        }
                                    )

                                    store.dispatch(coreActions.playlistLoaded(playlist));
                                })
                        }
                    })
                break;

            case 'MOPIDY_GET_PLAYLIST':
                request(socket, store, 'playlists.lookup', action.data )
                    .then(response => {
                        var playlist = Object.assign(
                            {},
                            response,
                            {
                                uri: response.uri,
                                type: 'playlist',
                                is_completely_loaded: true,
                                provider: 'mopidy',
                                tracks: (response.tracks ? response.tracks : []),
                                tracks_total: (response.tracks ? response.tracks.length : [])
                            }
                        )

                        // tracks? get the full track objects
                        if (playlist.tracks.length > 0){
                            store.dispatch({
                                type: 'MOPIDY_RESOLVE_PLAYLIST_TRACKS',
                                tracks: playlist.tracks,
                                key: playlist.uri
                            });
                        }

                        store.dispatch(coreActions.playlistLoaded(playlist));
                    })
                break;

            case 'MOPIDY_RESOLVE_PLAYLIST_TRACKS':
                var tracks = Object.assign([], action.tracks)
                var uris = helpers.arrayOf('uri',tracks)

                request(socket, store, 'library.lookup', { uris: uris })
                    .then(response => {
                        for (var uri in response){
                            if (response.hasOwnProperty(uri)){

                                var track = response[uri][0]
                                if (track){

                                    // find the track reference, and drop in the full track data
                                    function getByURI(trackReference){
                                        return track.uri == trackReference.uri
                                    }
                                    var trackReferences = tracks.filter(getByURI);

                                    // there could be multiple instances of this track, so accommodate this
                                    for(var j = 0; j < trackReferences.length; j++){
                                        var key = tracks.indexOf(trackReferences[j] );
                                        tracks[ key ] = track;
                                    }
                                }
                            }
                        }

                        store.dispatch({
                            type: 'PLAYLIST_TRACKS',
                            tracks: tracks,
                            key: action.key
                        });
                    })
                break

            case 'MOPIDY_ADD_PLAYLIST_TRACKS':

                request(socket, store, 'playlists.lookup', { uri: action.key })
                    .then(response => {
                        var tracks = [];
                        for(var i = 0; i < action.tracks_uris.length; i++){
                            tracks.push({
                                __model__: "Track",
                                uri: action.tracks_uris[i]
                            });
                        }

                        var playlist = Object.assign({}, response)
                        if (playlist.tracks){
                            playlist.tracks = [...playlist.tracks, ...tracks]
                        } else {
                            playlist.tracks = tracks
                        }

                        request(socket, store, 'playlists.save', { playlist: playlist } )
                            .then(response => {
                                store.dispatch({
                                    type: 'PLAYLIST_TRACKS_ADDED',
                                    key: action.key,
                                    tracks_uris: action.tracks_uris
                                });
                            })
                    });
                break

            case 'MOPIDY_REMOVE_PLAYLIST_TRACKS':

                // reverse order our indexes (otherwise removing from top will affect the keys following)
                function descending(a,b){
                    return b-a;
                }
                var indexes = Object.assign([], action.tracks_indexes)
                indexes.sort(descending);

                request(socket, store, 'playlists.lookup', { uri: action.key })
                    .then(response => {
                        var playlist = Object.assign({}, response)
                        for(var i = 0; i < indexes.length; i++){
                            playlist.tracks.splice(indexes[i], 1);
                        }
                        request(socket, store, 'playlists.save', { playlist: playlist } )
                            .then(response => {
                                store.dispatch({
                                    type: 'PLAYLIST_TRACKS_REMOVED',
                                    key: action.key,
                                    tracks_indexes: action.tracks_indexes
                                });
                            })
                    });
                break

            case 'MOPIDY_SAVE_PLAYLIST':
                var uri = action.key;
                request(socket, store, 'playlists.lookup', { uri: action.key })
                    .then(response => {
                        var playlist = Object.assign({}, response, { name: action.name })
                        request(socket, store, 'playlists.save', { playlist: playlist })
                            .then(response => {

                                store.dispatch(coreActions.playlistLoaded(playlist));

                                // When we rename a playlist, the URI also changes to reflect the name change
                                // We need to update our index, as well as redirect our current page URL
                                if (action.key != response.key){
                                    store.dispatch({
                                        type: 'PLAYLIST_KEY_UPDATED',
                                        key: action.key,
                                        new_key: response.uri
                                    });
                                    hashHistory.push(global.baseURL+'playlist/'+encodeURIComponent(response.uri));
                                }

                                store.dispatch(uiActions.createNotification({type: 'info', content: 'Playlist saved'}));
                            })
                    });
                break

            case 'MOPIDY_REORDER_PLAYLIST_TRACKS':
                request(socket, store, 'playlists.lookup', { uri: action.key })
                    .then(response => {

                        var playlist = Object.assign({}, response)
                        var tracks = Object.assign([], playlist.tracks)
                        var tracks_to_move = []

                        // calculate destination index: if dragging down, accommodate the offset created by the tracks we're moving
                        var range_start = action.range_start
                        var range_length = action.range_length
                        var insert_before = action.insert_before
                        if (insert_before > range_start ) insert_before = insert_before - range_length

                        // collate our tracks to be moved
                        for(var i = 0; i < range_length; i++){

                            // add to FRONT: we work backwards to avoid screwing up our indexes
                            tracks_to_move.unshift(tracks[range_start + i] )
                        }

                        // remove tracks from their old location
                        tracks.splice(range_start, range_length )

                        // now plug them back in, in their new location
                        for(var i = 0; i < tracks_to_move.length; i++){
                            tracks.splice(insert_before, 0, tracks_to_move[i] )
                        }

                        // update playlist
                        playlist = Object.assign({}, playlist, { tracks: tracks })
                        request(socket, store, 'playlists.save', { playlist: playlist } )
                            .then(response => {
                                store.dispatch({
                                    type: 'MOPIDY_RESOLVE_PLAYLIST_TRACKS',
                                    tracks: playlist.tracks,
                                    key: playlist.uri
                                });
                            });
                    });
                break

            case 'MOPIDY_CREATE_PLAYLIST':
                request(socket, store, 'playlists.create', { name: action.name, uri_scheme: action.scheme })
                    .then(response => {
                        store.dispatch(uiActions.createNotification({type: 'info', content: 'Created playlist'}))

                        store.dispatch(coreActions.playlistLoaded(response));

                        store.dispatch({
                            type: 'MOPIDY_LIBRARY_PLAYLIST_CREATED',
                            key: action.uri,
                            playlist: response
                        })
                    });
                break

            case 'MOPIDY_DELETE_PLAYLIST':
                request(socket, store, 'playlists.delete', { uri: action.uri })
                    .then(response => {
                        store.dispatch(uiActions.createNotification({content: 'Deleted playlist'}))
                        store.dispatch({
                            type: 'MOPIDY_LIBRARY_PLAYLIST_DELETED',
                            key: action.uri
                        })
                    });
                break


            /**
             * =============================================================== ALBUM(S) =============
             * ======================================================================================
             **/

            case 'MOPIDY_GET_LIBRARY_ALBUMS':
                var last_run = store.getState().ui.processes.MOPIDY_LIBRARY_ALBUMS_PROCESSOR

                if (!last_run){
                    request(socket, store, 'library.browse', { uri: store.getState().mopidy.library_albums_uri })
                        .then(response => {
                            if (response.length <= 0) return

                            var uris = helpers.arrayOf('uri',response)
                            store.dispatch({
                                type: 'MOPIDY_LIBRARY_ALBUMS_LOADED',
                                uris: uris
                            });

                            // Start our process to load the full album objects
                            store.dispatch(uiActions.startProcess(
                                'MOPIDY_LIBRARY_ALBUMS_PROCESSOR',
                                'Loading '+uris.length+' local albums',
                                {
                                    uris: uris,
                                    total: uris.length,
                                    remaining: uris.length
                                }
                            ))
                        })

                } else if (last_run.status == 'cancelled'){
                    store.dispatch(uiActions.resumeProcess('MOPIDY_LIBRARY_ALBUMS_PROCESSOR'))
                } else if (last_run.status == 'finished'){
                    // TODO: do we want to force a refresh?
                }

                break;

            case 'MOPIDY_LIBRARY_ALBUMS_PROCESSOR':
                if (store.getState().ui.processes['MOPIDY_LIBRARY_ALBUMS_PROCESSOR'] !== undefined){
                    var processor = store.getState().ui.processes['MOPIDY_LIBRARY_ALBUMS_PROCESSOR']

                    if (processor.status == 'cancelling'){
                        store.dispatch(uiActions.processCancelled('MOPIDY_LIBRARY_ALBUMS_PROCESSOR'))
                        return false
                    }
                }

                var uris = Object.assign([], action.data.uris);
                var uris_to_load = uris.splice(0,50);

                if (uris_to_load.length > 0){
                    store.dispatch(uiActions.updateProcess(
                        'MOPIDY_LIBRARY_ALBUMS_PROCESSOR',
                        'Loading '+uris.length+' local albums',
                        {
                            uris: uris,
                            remaining: uris.length
                        }
                    ));
                    store.dispatch(mopidyActions.getAlbums(uris_to_load, {name: 'MOPIDY_LIBRARY_ALBUMS_PROCESSOR', data: {uris: uris}}))
                } else {
                    store.dispatch(uiActions.processFinishing('MOPIDY_LIBRARY_ALBUMS_PROCESSOR'))
                }

                break

            case 'MOPIDY_GET_ALBUMS':
                request(socket, store, 'library.lookup', { uris: action.uris } )
                    .then(response => {
                        if (response.length <= 0){
                        	return;
                        }

                        var albums_loaded = [];
                    	var artists_loaded = [];
                    	var tracks_loaded = [];

                        for (var uri in response){
                            if (response.hasOwnProperty(uri) && response[uri].length > 0 && response[uri][0].album){
                                var tracks = response[uri];

                            	var artists_uris = [];
                                if (tracks[0].artists){
                                	for (var artist of response[uri][0].artists){
                                        artists_uris.push(artist.uri);
                                		artists_loaded.push(artist);
                                    }
                            	}

                            	var tracks_uris = [];
                            	for (var track of tracks){
                                    tracks_uris.push(track.uri);
                            		tracks_loaded.push(track);
                            	}

                                var album = Object.assign(
                                    {},
                                    {
                                        source: 'local',
                                        artists_uris: artists_uris,
                                        tracks_uris: tracks_uris,
                                        tracks_total: tracks_uris.length
                                    },
                                    tracks[0].album
                                )

                                albums_loaded.push(album);
                            }
                        }

                        store.dispatch(coreActions.albumsLoaded(albums_loaded));
                        store.dispatch(coreActions.artistsLoaded(artists_loaded));
                        store.dispatch(coreActions.tracksLoaded(tracks_loaded));

                        // Re-run any consequential processes in a few ms. This allows a small window for other
                        // server requests before our next batch. It's a little crude but it means the server isn't
                        // locked until we're completely done.
                        if (action.processor){
                            setTimeout(
                                function(){
                                    store.dispatch(uiActions.runProcess(action.processor.name, action.processor.data))
                                },
                                10
                            )
                        }
                    })
                break;

            case 'MOPIDY_GET_ALBUM':
                request(socket, store, 'library.lookup', action.data)
                    .then(response => {
                        if (!response || response.length <= 0){
                            return;
                        }

                        var artists = [];
                        if (response[0].artists){
                            for (var artist of response[0].artists){
                            	artists.push(artist);
                            }
                        }

                        var album = Object.assign(
                            {},
                            { 
                            	images: []
                            },
                            response[0].album,
                            {
                                source: 'local',
                                artists_uris: helpers.arrayOf('uri', artists),
                                tracks_uris: helpers.arrayOf('uri', response),
                                tracks_total: response.length
                            }
                        );
                        
                        store.dispatch(coreActions.albumLoaded(album));
                        store.dispatch(coreActions.artistsLoaded(artists));

                         // load artwork from LastFM
                        if (album.images.length <= 0){
                            var mbid = helpers.getFromUri('mbid',album.uri);
                            if (mbid){
                                store.dispatch(lastfmActions.getAlbum(false, false, mbid));
                            } else {
                                store.dispatch(lastfmActions.getAlbum(album.artists[0].name, album.name));
                            }
                        }

                        request(socket, store, 'library.lookup', { uris: album.tracks_uris } )
                            .then(response => {
                            	var tracks_loaded = [];

                                for (var uri in response){
                                    if (response.hasOwnProperty(uri)){
                                        tracks_loaded.push(response[uri][0]);
                                    }
                                }

                                store.dispatch(coreActions.tracksLoaded(tracks_loaded));
                            })
                    })
                break;


            /**
             * =============================================================== ARTIST(S) ============
             * ======================================================================================
             **/
            case 'MOPIDY_GET_LIBRARY_ARTISTS':
                request(socket, store, 'library.browse', { uri: store.getState().mopidy.library_artists_uri } )
                    .then(response => {
                        if (response.length <= 0) return

                        var uris = [];
                        for (var i = 0; i < response.length; i++){

                            // Convert local URI to actual artist URI
                            // See https://github.com/mopidy/mopidy-local-sqlite/issues/39
                            response[i].uri = response[i].uri.replace('local:directory?albumartist=','');
                            uris.push(response[i].uri);
                        }

                        store.dispatch(coreActions.artistsLoaded(response));

                        store.dispatch({
                            type: 'MOPIDY_LIBRARY_ARTISTS_LOADED',
                            uris: uris
                        });
                    });
                break;

            /**
             * TODO: Fetch and process library artists
             *
             * We can't get specific artist artwork from Mopidy. Perhaps we fetch additional
             * artist metadata via LastFM? Their API limits will make this quite slow.
            case 'MOPIDY_GET_LIBRARY_ARTISTS':

                var last_run = store.getState().ui.processes.MOPIDY_LIBRARY_ARTISTS_PROCESSOR

                if (!last_run){
                    request(socket, store, 'library.browse', { uri: 'local:directory?type=artist' } )
                        .then(response => {
                            if (response.length <= 0) return;

                            var uris = helpers.arrayOf('uri',response);

                            store.dispatch({
                                type: 'MOPIDY_LIBRARY_ARTISTS_LOADED',
                                uris: uris
                            });

                            // Start our process to load the full album objects
                            store.dispatch(uiActions.startProcess('MOPIDY_LIBRARY_ARTISTS_PROCESSOR','Loading '+uris.length+' local artists', {uris: uris}));
                        })

                } else if (last_run.status == 'cancelled'){
                    store.dispatch(uiActions.resumeProcess('MOPIDY_LIBRARY_ARTISTS_PROCESSOR'))
                } else if (last_run.status == 'finished'){
                    // TODO: do we want to force a refresh?
                }

                break;

            case 'MOPIDY_LIBRARY_ARTISTS_PROCESSOR':
                if (store.getState().ui.processes['MOPIDY_LIBRARY_ARTISTS_PROCESSOR'] !== undefined){
                    var processor = store.getState().ui.processes['MOPIDY_LIBRARY_ARTISTS_PROCESSOR'];

                    if (processor.status == 'cancelling'){
                        store.dispatch(uiActions.processCancelled('MOPIDY_LIBRARY_ARTISTS_PROCESSOR'));
                        return false;
                    }
                }

                var uris = Object.assign([], action.data.uris)
                var uris_to_load = uris.splice(0,50)

                if (uris_to_load.length > 0){
                    store.dispatch(uiActions.updateProcess('MOPIDY_LIBRARY_ARTISTS_PROCESSOR', 'Loading '+uris.length+' local artists', {uris: uris}));
                    store.dispatch(mopidyActions.getArtists(uris_to_load, {name: 'MOPIDY_LIBRARY_ARTISTS_PROCESSOR', data: {uris: uris}}));
                } else {
                    store.dispatch(uiActions.processFinishing('MOPIDY_LIBRARY_ARTISTS_PROCESSOR'));
                }

                break;
             **/

            case 'MOPIDY_GET_ARTIST':
                request(socket, store, 'library.lookup', action.data )
                    .then(response => {
                        if (response.length <= 0) return

                        var albums = [];
                        for (var i = 0; i < response.length; i++){
                            if (response[i].album){
                                var album = Object.assign(
                                    {},
                                    response[i].album,
                                    {
                                        uri: response[i].album.uri,
                                    }
                                );
                                if (album){
                                    function getByURI(albumToCheck){
                                        return album.uri == albumToCheck.uri
                                    }
                                    var existingAlbum = albums.find(getByURI);
                                    if (!existingAlbum){
                                        albums.push(album);
                                    }
                                }
                            }
                        }
                        if (albums){
                            store.dispatch(coreActions.albumsLoaded(albums));
                        }

                        var artist = Object.assign(
                            {},
                            (response ? response[0].artists[0] : {}),
                            {
                                is_mopidy: true,
                                albums_uris: helpers.arrayOf('uri',albums),
                                tracks: response.slice(0,10)
                            }
                        );
                        store.dispatch(coreActions.artistLoaded(artist));

                        // load artwork from LastFM
                        var existing_artist = store.getState().core.artists[artist.uri];
                        if (existing_artist && !existing_artist.images){
                            if (artist.musicbrainz_id){
                                store.dispatch(lastfmActions.getArtist(artist.uri, false, artist.musicbrainz_id));
                            } else {
                                store.dispatch(lastfmActions.getArtist(artist.uri, artist.name));
                            }
                        }
                    })
                break;

            case 'MOPIDY_GET_ARTISTS':
                request(socket, store, 'library.lookup', {uris: action.uris})
                    .then(response => {
                        if (response.length <= 0) return

                        var artists = []

                        for (var uri in response){
                            if (response.hasOwnProperty(uri) && response[uri].length > 0 && response[uri][0].artists){
                                var artist = Object.assign(
                                    {},
                                    (response ? response[uri][0].artists[0] : {}),
                                    {
                                        is_mopidy: true
                                    }
                                );
                                artists.push(artist);
                            }
                        }

                        store.dispatch(coreActions.artistsLoaded(artists));

                        // Re-run any consequential processes in 100ms. This allows a small window for other
                        // server requests before our next batch. It's a little crude but it means the server isn't
                        // locked until we're completely done.
                        if (action.processor){
                            setTimeout(
                                function(){
                                    store.dispatch(uiActions.runProcess(action.processor.name, action.processor.data))
                                },
                                100
                            )
                        }
                    })
                break;


            /**
             * =============================================================== TRACKS ================
             * ======================================================================================
             **/

            case 'MOPIDY_GET_QUEUE':
                request(socket, store, 'tracklist.getTlTracks')
                    .then(
                        response => {
                            store.dispatch({
                                type: 'QUEUE_LOADED',
                                tracks: response
                            });
                        }
                    );
                break;

            case 'MOPIDY_GET_QUEUE_HISTORY':
                request(socket, store, 'history.getHistory')
                    .then(
                        response => {
                            store.dispatch({
                                type: 'MOPIDY_QUEUE_HISTORY',
                                tracks: response
                            });
                        }
                    );
                break;

            case 'MOPIDY_GET_CURRENT_TRACK':
                request(socket, store, 'playback.getCurrentTlTrack')
                    .then(
                        response => {
                            if (response && response.track){
                                store.dispatch(mopidyActions.currentTrackLoaded(response));
                            }
                        }
                    );
                break;

            case 'MOPIDY_CURRENT_TRACK_LOADED':
                var track = helpers.formatTrack(action.tl_track);

                // We don't have the track already in our index, or we do but it's a partial record
                if (track.uri){
	                if (store.getState().core.tracks[track.uri] === undefined || store.getState().core.tracks[track.uri].images === undefined){

	                    // We've got Spotify running, and it's a spotify track - go straight to the source!
	                    if (store.getState().spotify.enabled && helpers.uriSource(track.uri) == 'spotify'){
	                        store.dispatch(spotifyActions.getTrack(track.uri));

	                    // Some other source, rely on Mopidy backends to do their work
	                    } else {
	                        store.dispatch(mopidyActions.getImages('tracks',[track.uri]));
	                    }
	                }

	                store.dispatch({
	                    type: 'CURRENT_TRACK_LOADED',
	                    track: track,
	                    uri: track.uri
	                });
	            }
                break;

            case 'MOPIDY_GET_NEXT_TRACK':
                request(socket, store, 'tracklist.getNextTlid')
                    .then(
                        response => {
                            if (response && response >= 0){

                                // Get the full track object from our tracklist
                                // We know it will be here, as the tlid refers to an item in this list
                                var track = helpers.applyFilter('tlid', response, store.getState().core.queue, true);

                                if (track && track.uri){
                                    store.dispatch({
                                        type: 'NEXT_TRACK_LOADED',
                                        uri: track.uri
                                    });

                                    // We don't have the track (including images) already in our index
                                    if (store.getState().core.tracks[track.uri] === undefined || store.getState().core.tracks[track.uri].images === undefined){

                                        // We've got Spotify running, and it's a spotify track - go straight to the source!
                                        if (store.getState().spotify.enabled && helpers.uriSource(track.uri) == 'spotify'){
                                            store.dispatch(spotifyActions.getTrack(track.uri));

                                        // Some other source, rely on Mopidy backends to do their work
                                        } else {
                                            store.dispatch(mopidyActions.getImages('tracks',[track.uri]));
                                        }
                                    }
                                }
                            }
                        }
                    );
                break;

            case 'MOPIDY_GET_TRACK':
                request(socket, store, 'library.lookup', action.data )
                    .then(
                        response => {
                            if (response.length > 0){
                                var track = Object.assign({}, response[0]);
                                store.dispatch(coreActions.trackLoaded(track));
                            }
                        },
                        error => {
                            store.dispatch(coreActions.handleException(
                                "Mopidy: "+(error.message ? error.message : "Could not get track"),
                                error
                            ));
                        }
                    )
                break


            /**
             * =============================================================== IMAGES ===============
             * ======================================================================================
             **/

            case 'MOPIDY_GET_IMAGES':
            	if (action.uris){
	                request(socket, store, 'library.getImages', {uris: action.uris})
	                    .then(response => {
	                        var records = [];
	                        for (var uri in response){
	                            if (response.hasOwnProperty(uri)){
	                                var images = response[uri];
	                                images = helpers.digestMopidyImages(store.getState().mopidy, images);

	                                if (images && images.length > 0){
	                                    records.push({
	                                        uri: uri,
	                                        images: images
	                                    });

	                                } else {
	                                    store.dispatch(lastfmActions.getImages(action.context, uri));
	                                }
	                            }
	                        }

	                        var action_data = {
	                            type: (action.context+'_LOADED').toUpperCase()
	                        }
	                        action_data[action.context] = records;
	                        store.dispatch(action_data);
	                    });
                }

                next(action);
                break;


            /**
             * =============================================================== LOCAL ================
             * ======================================================================================
             **/

            case 'MOPIDY_GET_DIRECTORY':
                store.dispatch({
                    type: 'MOPIDY_DIRECTORY_FLUSH'
                });

                request(socket, store, 'library.browse', action.data)
                    .then(response => {

                        var tracks_uris = [];
                        var subdirectories = [];

                        for (var item of response){
                            if (item.type === "track"){
                                tracks_uris.push(item.uri);
                                tracks_uris = helpers.sortItems(tracks_uris, 'name');
                            } else {
                                subdirectories.push(item);
                                subdirectories = helpers.sortItems(subdirectories, 'name');
                            }
                        }
                        
                        if (tracks_uris.length > 0){
                            request(socket, store, 'library.lookup', {uris: tracks_uris})
                                .then(response => {
                                    if (response.length <= 0){
                                        return;
                                    }

                                    var tracks = [];

                                    for (var uri in response){
                                        if (response.hasOwnProperty(uri) && response[uri].length > 0){
                                            tracks.push(helpers.formatTrack(response[uri][0]));
                                        }
                                    }

                                    store.dispatch({
                                        type: 'MOPIDY_DIRECTORY_LOADED',
                                        directory: {
                                            tracks: tracks,
                                            subdirectories: subdirectories
                                        }
                                    });
                                }
                            );
                        } else {
                            store.dispatch({
                                type: 'MOPIDY_DIRECTORY_LOADED',
                                directory: {
                                    tracks: tracks,
                                    subdirectories: subdirectories
                                }
                            });
                        }
                    });
                break;

            case 'MOPIDY_DIRECTORY':
                if (store.getState().ui.allow_reporting && action.data){
                	ReactGA.event({ category: 'Directory', action: 'Load', label: action.data.uri });
                }
                next(action)
                break

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action)
        }
    }

})();

export default MopidyMiddleware

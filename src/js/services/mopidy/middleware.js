
import Mopidy from 'mopidy'
import actions from './actions'

const MopidyMiddleware = (function(){ 

    // container for the actual Mopidy socket
    var socket = null;

    // handle all manner of socket messages
    const handleMessage = (ws, store, type, data) => {
        switch( type ){

            case 'state:online':
                store.dispatch({ type: 'MOPIDY_CONNECTED' });
                instruct( ws, store, 'playback.getState' );
                instruct( ws, store, 'playback.getVolume' );
                instruct( ws, store, 'tracklist.getConsume' );
                instruct( ws, store, 'tracklist.getRandom' );
                instruct( ws, store, 'tracklist.getRepeat' );
                instruct( ws, store, 'tracklist.getTlTracks' );
                instruct( ws, store, 'playback.getCurrentTlTrack' );
                break;

            case 'state:offline':
                store.dispatch({ type: 'MOPIDY_DISCONNECTED' });
                break;

            case 'event:tracklistChanged':
                instruct( ws, store, 'tracklist.getTlTracks' );
                break;

            //case 'event:trackPlaybackEnded':
            case 'event:playbackStateChanged':
            case 'event:trackPlaybackStarted':
                instruct( ws, store, 'playback.getState' );
                instruct( ws, store, 'playback.getCurrentTlTrack' );
                break;

            case 'event:volumeChanged':
                store.dispatch({ type: 'MOPIDY_Volume', data: data.volume });
                break;

            case 'event:optionsChanged':
                instruct( ws, store, 'tracklist.getConsume' );
                instruct( ws, store, 'tracklist.getRandom' );
                instruct( ws, store, 'tracklist.getRepeat' );
                break;

            default:
                //console.log( 'MopidyService: Unhandled message', type, message );
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
     **/
    const instruct = ( ws, store, call, value = false ) => {

        var callParts = call.split('.');
        var model = callParts[0];
        var method = callParts[1];
        var property = method;
        property = property.replace('get','');
        property = property.replace('set','');

        // if we have a value, we need to include in our payload
        if( value ){
            ws[model][method]( value )
                .then(
                    response => store.dispatch({ type: 'MOPIDY_'+property, model: model, data: response }),
                    error => console.error( error )
                );

        // no value, so omit the payload
        }else{
            ws[model][method]()
                .then(
                    response => store.dispatch({ type: 'MOPIDY_'+property, model: model, data: response }),
                    error => console.error( error )
                );
        }
    }

    /**
     * Middleware
     *
     * This behaves like an action interceptor. We listen for specific actions
     * and handle special functionality. If the action is not in our switch, then
     * it just proceeds to the next middleware, or default functionality
     **/
    return store => next => action => {
        switch(action.type) {

            case 'MOPIDY_CONNECT':

                if(socket != null) socket.close();
                store.dispatch({ type: 'MOPIDY_CONNECTING' });

                var state = store.getState();

                socket = new Mopidy({
                    webSocketUrl: 'ws://'+state.mopidy.host+':'+state.mopidy.port+'/mopidy/ws',
                    callingConvention: 'by-position-or-by-name'
                });

                socket.on( (type, data) => handleMessage( socket, store, type, data ) );

                break;

            case 'MOPIDY_DISCONNECT':
                if(socket != null) socket.close();
                socket = null;                
                store.dispatch({ type: 'MOPIDY_DISCONNECTED' });
                break;

            // send an instruction to the websocket
            case 'MOPIDY_INSTRUCT':
            case 'MOPIDY_CHANGE_TRACK':
            case 'MOPIDY_REMOVE_TRACKS':
                instruct( socket, store, action.call, action.value )
                break;

            case 'MOPIDY_SET_CONFIG':
                next(action);
                localStorage.setItem('mopidy', JSON.stringify({ host: action.host, port: action.port }));
                break;

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default MopidyMiddleware
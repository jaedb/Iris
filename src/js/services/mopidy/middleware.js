
import Mopidy from 'mopidy'
import actions from './actions'

const MopidyMiddleware = (function(){ 

    var socket = null;

    const onOpen = (ws, store, token) => evt => {
        console.log('opened')
        //Send a handshake, or authenticate with remote end

        //Tell the store we're connected
        //store.dispatch(actions.connected());
    }

    const onClose = (ws, store) => evt => {
        //Tell the store we've disconnected
        //store.dispatch(actions.disconnected());
    }

    const handleMessage = (ws, store, type, data) => {

        switch( type ){

            case 'state:online':
                //store.dispatch( actions.updateStatus( true ) );
                instruct( ws, store, 'playback.getState' );
                instruct( ws, store, 'playback.getVolume' );
                instruct( ws, store, 'tracklist.getConsume' );
                instruct( ws, store, 'tracklist.getRandom' );
                instruct( ws, store, 'tracklist.getRepeat' );
                instruct( ws, store, 'tracklist.getTlTracks' );
                instruct( ws, store, 'playback.getCurrentTlTrack' );
                break;

            case 'state:offline':
                store.dispatch( actions.updateStatus( false ) );
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
                store.dispatch( actions.updateVolume( data.volume ) );
                break;

            case 'event:optionsChanged':
                instruct( ws, store, 'tracklist.getConsume' );
                instruct( ws, store, 'tracklist.getRandom' );
                instruct( ws, store, 'tracklist.getRepeat' );
                break;

            default:
                //console.log( 'MopidyService: Unhandled event', type, message );
        }
    }


    function doStuff( ws, store ){
        console.log('doing stuff', store);
        ws.playback.getState()
            .then(
                response => {
                    store.dispatch({ type: 'STATE', state: response })
                },
                error => {
                    console.error( error );
                }
            );
    }


    /**
     * Get something from Mopidy
     *
     * Sends request to Mopidy server, and updates our local storage on return
     * @param object ws Websocket
     * @param string model Mopidy model (playback, tracklist, etc)
     * @param string property the property to get (TlTracks, Consume, etc)
     **/
    const instruct = ( ws, store, call, value = false ) => {
        console.log('Mopidy: '+call, value);
        var callParts = call.split('.');
        var model = callParts[0];
        var method = callParts[1];
        var property = method;
        property = property.replace('get','');
        property = property.replace('set','');

        if( value ){
            ws[model][method]( value )
                .then(
                    response => store.dispatch({ type: property, model: model, data: response }),
                    error => console.error( error )
                );
        }else{
            ws[model][method]()
                .then(
                    response => store.dispatch({ type: property, model: model, data: response }),
                    error => console.error( error )
                );
        }
    }

    return store => next => action => {

        switch(action.type) {

            //The user wants us to connect
            case 'CONNECT':

                if(socket != null) socket.close();
                store.dispatch({ type: 'CONNECTING' });

                socket = new Mopidy({
                    webSocketUrl: "ws://tv.barnsley.nz:6680/mopidy/ws",
                    callingConvention: 'by-position-or-by-name'
                });

                socket.on( (type, data) => handleMessage( socket, store, type, data ) );

                break;

            //The user wants us to disconnect
            case 'DISCONNECT':
                if(socket != null) socket.close();
                socket = null;                
                store.dispatch({ type: 'DISCONNECTED' });
                break;

            //Send the 'SEND_MESSAGE' action down the websocket to the server
            case 'INSTRUCT':
                instruct( socket, store, action.call, action.value )
                break;

            //This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action);
        }
    }

})();

export default MopidyMiddleware
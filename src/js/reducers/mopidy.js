
export default function reducer(mopidy = {}, action){
    switch (action.type) {

        case 'STATUS':
            return Object.assign({}, mopidy, {
                online: action.online   
            });

        case 'STATE':
            return Object.assign({}, mopidy, {
                state: action.state 
            });

        case 'CONSUME':
            return Object.assign({}, mopidy, {
                consume: action.consume 
            });

        case 'RANDOM':
            return Object.assign({}, mopidy, {
                random: action.random 
            });

        case 'REPEAT':
            return Object.assign({}, mopidy, {
                repeat: action.repeat 
            });

        case 'TRACKLIST':
            return Object.assign({}, mopidy, {
            	tracks: action.tracks	
            });

        case 'TRACKINFOCUS':
        	console.log( action );
            return Object.assign({}, mopidy, {
            	trackInFocus: action.trackInFocus	
            });

        case 'VOLUME':
            return Object.assign({}, mopidy, {
                volume: action.volume   
            });

        case 'CHANGE_TRACK':
            return Object.assign({}, mopidy, {
            	tlid: action.tlid	
            });

        default:
            return mopidy
    }
}




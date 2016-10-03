
export default function reducer(mopidy = {}, action){
    switch (action.type) {

        case 'STATUS':
            return Object.assign({}, mopidy, {
            	online: action.online	
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

        default:
            return mopidy
    }
}




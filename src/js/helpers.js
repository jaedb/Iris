

export let isTouchDevice = function(){
	return 'ontouchstart' in document.documentElement
}

export let sizedImages = function( images ){

	var sizes = {
		small: false,
		medium: false,
		large: false,
		huge: false
	}

	if( images.length <= 0 ) return sizes;

	for( var i = 0; i < images.length; i++ ){
		
		// Mopidy image object
		if (typeof(images[i].__model__) !== 'undefined' && images[i].__model__ == 'Image'){
			sizes.small = images[i].uri

		// Mopidy image string
		} else if (typeof(images[i]) == 'string'){
			sizes.small = images[i]

		// spotify-styled images
		} else if (typeof(images[i].width) !== 'undefined'){
			if( images[i].width < 400 ){
				sizes.small = images[i].url;
			}else if( images[i].width < 800 ){
				sizes.medium = images[i].url;
			}else if( images[i].width < 1000 ){
				sizes.large = images[i].url;
			}else{
				sizes.huge = images[i].url;
			}

		// lastfm-styled images
		} else if (typeof(images[i].size) !== 'undefined'){
			switch( images[i].size ){
				case 'mega':
				case 'extralarge':
					sizes.huge = images[i]['#text']
					break
				case 'large':
					sizes.large = images[i]['#text']
					break
				case 'medium':
					sizes.medium = images[i]['#text']
					break
				case 'small':
					sizes.small = images[i]['#text']
					break
			}
		}
	}

	if (!sizes.medium){
		if (sizes.large) sizes.medium = sizes.large
		else if (sizes.huge) sizes.medium = sizes.huge
		else sizes.medium = sizes.small
	}
	if (!sizes.large) sizes.large = sizes.medium;
	if (!sizes.huge) sizes.huge = sizes.large;
	
	return sizes;
}

export let generateGuid = function(format = 'xxxxxxxxxxxx'){
	return format.replace(/[xy]/g, function(c) {
		var r = Math.random()*16|0, v = c == 'x' ? r : (r&0x3|0x8);
		return v.toString(16);
	});
}


export let getCurrentPusherConnection = function( connections, connectionid ){
	function isCurrentConnection(connection){
		return connection.connectionid == newProps.pusher.connectionid;
	}
	
	var currentConnection = newProps.pusher.connections.find(isCurrentConnection);
	if( !currentConnection ) return false;

	return currentConnection;
}


/**
 * Get a track's icon
 * @param track object
 * @return string
 **/
export let getTrackIcon = function( track = false ){
	if( !track ) return false
	if( !track.album ) return false
	if( !track.album.images ) return false
	return sizedImages( track.album.images ).small
}


/**
 * Get a track's icon
 * @param track object
 * @return string
 **/
export let flattenTracks = function( tracks ){
    var flattened = []
    for( var i = 0; i < tracks.length; i++ ){
        flattened.push( Object.assign(
            {},
            tracks[i].track,
            {
                added_by: tracks[i].added_by,
                added_at: tracks[i].added_at
            }
        ))
    }

    return flattened
}




/**
 * Figure out a URI's source namespace
 * @param uri = string
 **/
export let uriSource = function(uri){
    var exploded = uri.split(':');
    return exploded[0]
}

export let sourceIcon = function(uri,source = null){
	if (uri) source = uriSource(uri)
	switch( source ){
		case 'local':
		case 'm3u':
			return 'folder'
			break
		case 'gmusic':
			return 'google'
			break
		default:
			return source
	}
}



/**
 * Get an element from a URI
 * @param element = string, the element we wish to extract
 * @param uri = string
 **/
export let getFromUri = function(element,uri){
    var exploded = uri.split(':');
    var namespace = exploded[0]

    switch (element){
    	case 'mbid':
	        var index = exploded.indexOf('mbid')
	        if( index > -1 ) return exploded[index+1]
	        break

    	case 'artistid':
    		if (exploded[1] == 'artist'){
    			return exploded[2]
    		}
    		break

    	case 'albumid':
    		if (exploded[1] == 'album'){
    			return exploded[2]
    		}
    		break

    	case 'playlistid':
    		if (exploded[1] == 'playlist'){
    			return exploded[2]
    		} else if (exploded[1] == 'user' && exploded[3] == 'playlist'){
    			return exploded[4]
    		}
    		break

    	case 'playlistowner':
    		if (exploded[1] == 'user' && exploded[3] == 'playlist'){
    			return exploded[2]
    		}
    		break

    	case 'trackid':
    		if (exploded[1] == 'track'){
    			return exploded[2]
    		}
    		break

    	case 'userid':
    		if (exploded[1] == 'user'){
    			return exploded[2]
    		}
    		break

    	case 'genreid':
    		if (exploded[1] == 'genre'){
    			return exploded[2]
    		}
    		break

    	case 'seeds':
    		if (exploded[1] == 'discover'){
    			return exploded[2]
    		}
    		break
    }
    return null
}

/**
 * Identify what kind of asset a URI is (playlist, album, etc)
 * @param uri = string
 * @return string
 **/
export let uriType = function( uri ){
    var exploded = uri.split(':')

    if (exploded[0] == 'm3u'){
    	return 'playlist'
    }

    switch (exploded[1]){
    	case 'track':
    	case 'artist':
    	case 'album':
    	case 'playlist':
    	case 'search':
    	case 'genre':
    	case 'discover':
    		return exploded[1]
    		break

    	case 'user':
    		if (exploded.length > 3 && exploded[3] == 'playlist'){
    			return 'playlist'
    		}
    		return exploded[1]
    		break
    }

    return null;
}


/**
 * Convert a raw URI into a object index-friendly format. Primarily used for loading local playlists
 * @param $uri = string
 * @return string
 **/
export let indexFriendlyUri = function (uri){
	var output = encodeURI(uri)
	output = output.replace("'",'%27')
	return output
}


/**
 * Digest an array of objects, and pull into simple array of uris
 * 
 * @param items Array
 * @return Array
 **/
export let asURIs = function(items){
	var uris = []
	for( var i = 0; i < items.length; i++ ){
		uris.push( items[i].uri )
	}
	return uris
}


/**
 * Merge duplicated items in an array
 *
 * @param list Array the unclean array
 * @param key string = the unique key (id, uri, tlid, etc)
 **/
export let mergeDuplicates = function(list, key){
	var clean_list = [];
	var keyed_list  = {};

	for( var i in list ){
		var item = list[i]
		if( item[key] in keyed_list ){
			item = Object.assign({}, keyed_list[item[key]], item)
		}
		keyed_list[item[key]] = item;
	}

	for( i in keyed_list ){
		clean_list.push(keyed_list[i]);
	}

	return clean_list;
}


/**
 * Remove duplicate items in a simple array
 *
 * @param list Array the unclean array
 **/
export let removeDuplicates = function(array){
	var unique = [];

	for( var i in array ){
		if (unique.indexOf(array[i]) <= -1 ){
			unique.push(array[i])
		}
	}

	return unique;
}


/**
 * Convert a list of indexes to a useable range
 * We ignore stragglers, and only attend to the first 'bunch' of consecutive indexes
 * 
 * @param indexes array of int
 **/
export let createRange = function (indexes){

	// sort our indexes smallest to largest
	function sortAsc(a,b){
        return a - b
    }
    indexes.sort(sortAsc);

    // iterate indexes to build the first 'bunch'
    var first_bunch = []
    var previous_index = false
    for( var i = 0; i < indexes.length; i++ ){
        if( !previous_index || previous_index == indexes[i]-1 ){
            first_bunch.push(indexes[i])
            previous_index = indexes[i]
        }
        // TODO: break when we find an integer step for better performance
    }

    return {
    	start: first_bunch[0],
    	length: first_bunch.length
    }
}



/**
 * Sort an array of objects
 * @param array = array to sort
 * @param property = string to sort by
 * @param reverse = boolean
 * @return array
 **/
export let sortItems = function (array, property, reverse = false){

	function compare(a,b) {

		var a_value = a
		var a_property_split = property.split('.')
		for (var i = 0; i < a_property_split.length; i++) {
			if (typeof(a_value[a_property_split[i]]) === 'undefined') {
				a_value = false
				break
			}else{
				a_value = a_value[a_property_split[i]]
			}
		}

		var b_value = b
		var b_property_split = property.split('.')
		for( var i = 0; i < b_property_split.length; i++ ){
			if (typeof(b_value[b_property_split[i]]) === 'undefined') {
				b_value = false
				break
			} else {
				b_value = b_value[b_property_split[i]]
			}
		}

		if( typeof(a_value) === 'boolean'){
			if (a_value && !b_value) return -1
			if (!a_value && b_value) return 1
			return 0

		}else if( typeof(a_value) === 'string'){
			if ( !a_value || !b_value ) return 0
			if (a_value.toLowerCase() > b_value.toLowerCase()) return 1
			if (a_value.toLowerCase() < b_value.toLowerCase()) return -1
			return 0

		}else{
			if (parseInt(a_value) > parseInt(b_value)) return 1
			if (parseInt(a_value) < parseInt(b_value)) return -1
			return 0
		}
	}

	var sorted = Object.assign([], array.sort(compare))
	if( reverse ) sorted.reverse()
	return sorted
}

/**
 * Figure out if a value is a number
 * @param data = mixed
 * @return boolean
 **/
export let isNumeric = function (data) {
	return !isNaN(parseFloat(data)) && isFinite(data)
}


/**
 * Set window title
 * @param track
 **/
export let setWindowTitle = function (track = false, play_state = false){

    var title = 'No track playing'
    
    if (track) {
        var icon = '\u25A0 '
        var artistString = ''
        
        if (track.artists) {
            for( var i = 0; i < track.artists.length; i++) {
                if (artistString != '') artistString += ', '
                artistString += track.artists[i].name
            }
        }

        if (play_state && play_state == 'playing') icon = '\u25B6 '

        title = icon +' '+ track.name +' - '+ artistString
    }
    
    document.title = title
}


/**
 * Detect if an item is in the loading queue. We simply loop all load items to
 * see if any items contain our searched key.
 *
 * TODO: Explore performance of this
 * TODO: Allow wildcards
 *
 * @param load_queue = obj (passed from store)
 * @param key = string (the string to lookup)
 * @return boolean
 **/
export let isLoading = function(load_queue = [], keys = []){

	// Loop all of our load queue items
	for (var load_queue_key in load_queue) {

		// Make sure it's not a root object method
		if (load_queue.hasOwnProperty(load_queue_key)){

			// Loop all the keys we're looking for
			for (var i = 0; i < keys.length; i++){
				if (load_queue[load_queue_key].includes(keys[i])){
					return true
				}
			}
		}
	}
	return false
}
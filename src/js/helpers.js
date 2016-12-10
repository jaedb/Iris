

export let sizedImages = function( images ){

	var sizes = {
		small: false,
		medium: false,
		large: false,
		huge: false
	}

	if( images.length <= 0 ) return sizes;

	for( var i = 0; i < images.length; i++ ){

		// spotify-styled images
		if( typeof(images[i].height) !== 'undefined' ){
			if( images[i].height < 400 ){
				sizes.small = images[i].url;
			}else if( images[i].height < 800 ){
				sizes.medium = images[i].url;
			}else if( images[i].height < 1000 ){
				sizes.large = images[i].url;
			}else{
				sizes.huge = images[i].url;
			}

		// lastfm-styled images
		}else if( typeof(images[i].size) !== 'undefined' ){
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

		// Mopidy-Images styled images
		}else if( typeof(images[i]) == 'string' ){
			sizes.small = images[i]
		}
	}

	if( !sizes.medium ){
		if( sizes.large ) sizes.medium = sizes.large
		else if( sizes.huge ) sizes.medium = sizes.huge
		else sizes.medium = sizes.small
	}
	if( !sizes.large ) sizes.large = sizes.medium;
	if( !sizes.huge ) sizes.huge = sizes.large;
	
	return sizes;
}

export let generateGuid = function(){
	return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
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
 * Figure out a URI's source namespace
 * @param uri = string
 **/
export let uriSource = function( uri ){
    var exploded = uri.split(':');
    return exploded[0]
}

export let sourceIcon = function( uri ){
	var source = uriSource(uri)
	switch( source ){
		case 'local':
		case 'm3u':
			return 'folder'
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
export let getFromUri = function( element, uri ){
    var exploded = uri.split(':');

    if( element == 'mbid'){
        var index = exploded.indexOf('mbid')
        if( index > -1 ) return exploded[index+1]
    }

    if( exploded[0] == 'spotify' ){
	    if( element == 'userid' && exploded[1] == 'user' ) return exploded[2];
	    if( element == 'playlistid' && exploded[3] == 'playlist' ) return exploded[4];
	    if( element == 'artistid' && exploded[1] == 'artist' ) return exploded[2];
	    if( element == 'albumid' && exploded[1] == 'album' ) return exploded[2];
	    if( element == 'trackid' && exploded[1] == 'track' ) return exploded[2];
	    return null;
	}

	return null
}

/**
 * Identify what kind of asset a URI is (playlist, album, etc)
 * @param uri = string
 * @return string
 **/
export let uriType = function( uri ){
    var exploded = uri.split(':');

    if( exploded[0] == 'spotify' ){
    	switch( exploded[1] ){
    		case 'track':
    			return 'track'
    			break;
    		case 'artist':
    			return 'artist'
    			break;
    		case 'album':
    			return 'album'
    			break;
    		case 'user':
    			if( exploded[3] == 'playlist' ) return 'playlist'
    			if( exploded.length == 3 ) return 'user'
    			return null
    			break;
    	}
    }

    return null;
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
 * @param key string = the unique key (uri, tlid, etc)
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
		for( var i = 0; i < a_property_split.length; i++ ){
			if( typeof(a_value[a_property_split[i]]) === 'undefined' ) return -1
			a_value = a_value[a_property_split[i]]
		}

		var b_value = b
		var b_property_split = property.split('.')
		for( var i = 0; i < b_property_split.length; i++ ){
			if( typeof(b_value[b_property_split[i]]) === 'undefined' ) return -1
			b_value = b_value[b_property_split[i]]
		}

		if( typeof(a_value) === 'boolean'){
			return a_value

		}else if( typeof(a_value) === 'string'){
			if(a_value.toLowerCase() > b_value.toLowerCase()) return 1
			return -1

		}else{
			if( parseInt(a_value) > parseInt(b_value) ) return 1
			return -1
		}
	}

	var sorted = array.sort(compare)
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
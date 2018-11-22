

export let isTouchDevice = function(){
	return 'ontouchstart' in document.documentElement
}

/**
 * Returns a function, that, as long as it continues to be invoked, will not
 * be triggered. The function will be called after it stops being called for
 * N milliseconds. If `immediate` is passed, trigger the function on the
 * leading edge, instead of the trailing.
 **/
export let debounce = function(fn, wait, immediate){
	var timeout;
	return function(){
		var context = this, args = arguments;

		var later = function(){
			timeout = null;
			if (!immediate){
				fn.apply(context, args);
			}
		};

		var callNow = immediate && !timeout;
		clearTimeout(timeout);
		timeout = setTimeout(later, wait);

		if (callNow){
			fn.apply(context, args);
		}
	};
};


export let throttle = function(fn, delay){
	let lastCall = 0;
	return function (...args){
		const now = (new Date).getTime();
		if (now - lastCall < delay) {
			return;
		}
		lastCall = now;
		return fn(...args);
	}
}


/**
 * Storage handler
 * All localStorage tasks are handled below. This means we can detect for localStorage issues in one place
 **/

var storage = (function() {
	var uid = new Date;
	var storage;
	var result;
	try {
		(storage = window.localStorage).setItem(uid, uid);
		result = storage.getItem(uid) == uid;
		storage.removeItem(uid);
		return result && storage;
	} catch (exception) {}
}());

/**
 * Get a storage value
 *
 * @param key = string
 * @param default_value = mixed (optional, if localStorage key doesn't exist, return this)
 **/
export let getStorage = function(key, default_value = {}){
	if (storage){
		var value = storage.getItem(key);
		if (value){
			return JSON.parse(value);
		} else {
			return default_value;
		}

	} else {
		console.warn("localStorage not available. Using default value for '"+key+"'.");
		return default_value;
	}
}

/**
 * Set a storage value
 *
 * @param key = string
 * @param value = object
 * @param replace = boolean (optional, completely replace our local value rather than merging it)
 **/
export let setStorage = function(key, value, replace = false){
	if (storage){
		var stored_value = storage.getItem(key);

		// We have nothing to merge with, or we want to completely replace previous value
		if (!stored_value || replace){
			var new_value = value;

		// Merge new value with existing
		} else {
			var new_value = Object.assign(
				{},
				JSON.parse(stored_value),
				value
			);
		}
		storage.setItem(key, JSON.stringify(new_value));
		return;
	} else {
		console.warn("localStorage not available. '"+key+"'' will not perist when you close your browser.");
		return;
	}
}



/**
 * Digest an array of Mopidy image objects into a universal format. We also re-write
 * image URLs to be absolute to the mopidy server (required for proxy setups).
 *
 * @param mopidy = obj (mopidy store object)
 * @param images = array
 * @return array
 **/
export let digestMopidyImages = function(mopidy, images){
	var digested = [];

	for (var i = 0; i < images.length; i++){

		// Image object (ie from images.get)
		if (typeof images[i] === 'object'){
			// Accommodate backends that provide URIs vs URLs
			var url = images[i].url
			if (!url && images[i].uri){
				url = images[i].uri
			}

	        // Amend our URL
	        images[i].url = url		

			// Replace local images to point directly to our Mopidy server
	        if (url && url.startsWith('/images/')){
	            url = '//'+mopidy.host+':'+mopidy.port+url
	        }

	    // String-based image
		} else if (typeof images[i] === 'string'){
			// Replace local images to point directly to our Mopidy server
	        if (images[i].startsWith('/images/')){
	            images[i] = '//'+mopidy.host+':'+mopidy.port+images[i]
	        }
		}

        digested.push(images[i])
	}

	return digested
}


export let generateGuid = function(type = 'numeric'){
	// numeric
	if (type == 'numeric'){
		var date = new Date().valueOf().toString();
		var random_number = Math.floor((Math.random() * 100)).toString();
		return parseInt(date+random_number);
	} else {
		var format = 'xxxxxxxxxx';
		return format.replace(/[xy]/g, function(c){
			var r = Math.random()*16|0, v = c == 'x' ? r : (r&0x3|0x8);
			return v.toString(16);
		});
	}
}

export let getCurrentPusherConnection = function(connections, connectionid){
	function isCurrentConnection(connection){
		return connection.connectionid == newProps.pusher.connectionid;
	}
	
	var currentConnection = newProps.pusher.connections.find(isCurrentConnection);
	if (!currentConnection ) return false;

	return currentConnection;
}


/**
 * Get a track's icon
 * @param track object
 * @return string
 **/
export let getTrackIcon = function(current_track = false, core = false){
	if (!core) return false
	if (!current_track) return false
	if (typeof(current_track.uri) == 'undefined') return false
	if (typeof(core.tracks[current_track.uri]) === 'undefined') return false
	var track = core.tracks[current_track.uri]
	if (!track.images) return false
	return formatImages(track.images).small
}

/**
 * Format image URLs into a consistent size-based object
 * We digest all our known image source formats into a universal small,medium,large,huge object
 *
 * @param $data mixed
 * @return Object
 **/
export let formatImages = function(data){

	var sizes = {
		formatted: true,
		small: null,
		medium: null,
		large: null,
		huge: null
	}

	if (!data){
		return sizes;
	}

	// An array of images has been provided
	if (Array.isArray(data)){

		if (data.length <= 0){
			return sizes;
		}

		for (var i = 0; i < data.length; i++){
			let image = data[i]

			// Mopidy image object
			if (image.__model__ && image.__model__ == 'Image'){

				if (image.width < 400){
					sizes.small = image.url;
				}else if (image.width < 800){
					sizes.medium = image.url;
				}else if (image.width < 1000){
					sizes.large = image.url;
				} else {
					sizes.huge = image.url;
				}

			// Mopidy image string
			} else if (typeof(image) == 'string'){
				sizes.small = image
			
			// spotify-styled images
			} else if (image.width !== undefined){

				if (image.width < 400){
					sizes.small = image.url;
				}else if (image.width < 800){
					sizes.medium = image.url;
				}else if (image.width < 1000){
					sizes.large = image.url;
				} else {
					sizes.huge = image.url;
				}

			// lastfm-styled images
			} else if (image.size !== undefined){
				switch(image.size){
					case 'mega':
					case 'extralarge':
					case 'large':
						sizes.medium = image['#text']
						break;
					case 'medium':
					case 'small':
						sizes.small = image['#text']
						break;
				}
			}
		}

	// An object of images has been provided
	// The Genius avatar object is an example of this 
	} else {
		if (data.small) sizes.small = data.small.url;
		if (data.medium) sizes.medium = data.medium.url;
		if (data.large) sizes.large = data.large.url;
		if (data.huge) sizes.huge = data.huge.url;
	}

	// Inherit images where we haven't been given the appropriate size
	// Ie small duplicated to tiny, large duplicated to medium, etc
	if (!sizes.small){
		if (sizes.medium) sizes.small = sizes.medium
		else if (sizes.large) sizes.small = sizes.large
		else if (sizes.huge) sizes.small = sizes.huge
		else sizes.small = null
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


/**
 * Format a simple object
 * This is a shell record containing only the bare essentials. Typically
 * a tracks' artists/album
 *
 * @param data obj
 * @return obj
 **/
export let formatSimpleObject = function(data){
	var simple_object = {}
	var fields = [
		'uri',
		'name'
	];

	for (var field of fields){
		if (data.hasOwnProperty(field)){
			simple_object[field] = data[field];
		}
	}

	return simple_object;
}

/**
 * Format multiple items
 *
 * @param tracks Array
 * @return Array
 **/
export let formatTracks = function(records = []){
    var formatted = [];
    for (var record of records){
	    formatted.push(formatTrack(record));
    }
    return formatted;
}
export let formatAlbums = function(records = []){
    var formatted = [];
    for (var record of records){
	    formatted.push(formatAlbum(record));
    }
    return formatted;
}
export let formatArtists = function(records = []){
    var formatted = [];
    for (var record of records){
	    formatted.push(formatArtist(record));
    }
    return formatted;
}
export let formatPlaylists = function(records = []){
    var formatted = [];
    for (var record of records){
	    formatted.push(formatTrack(record));
    }
    return formatted;
}
export let formatUsers = function(records = []){
    var formatted = [];
    for (var record of records){
	    formatted.push(formatUser(record));
    }
    return formatted;
}


/**
 * Format our album objects into a universal format
 *
 * @param data obj
 * @return obj
 **/
export let formatAlbum = function(data){
	var album = {};
	var fields = [
		'uri',
		'provider',
		'name',
		'type',
		'added_at',
		'release_date',
		'popularity',
		'images',
		'artists_uris',
		'tracks_uris',
		'artists'	// Array of simple records
	];

	// Loop fields and import from data
	for (var field of fields){
		if (data.hasOwnProperty(field)){
			album[field] = data[field];
		}
	}

	if (album.images && !album.images.formatted){
		album.images = formatImages(album.images);
	}

	if (data.date && !album.date){
		album.release_date = data.date;
	}
	if (data.album_type){
		album.type = data.album_type;
	}    
	if (album.provider === undefined && album.uri !== undefined){
		album.provider = uriSource(album.uri);
	}

	return album;
}


/**
 * Format our artist objects into a universal format
 *
 * @param data obj
 * @return obj
 **/
export let formatArtist = function(data){
	var artist = {}
	var fields = [
		'uri',
		'provider',
		'mbid',
		'name',
		'type',
		'images',
		'popularity',
		'followers',
		'listeners',
		'added_at',
		'biography',
		'biography_link',
		'biography_publish_date',
		'related_artists_uris',
		'albums_uris',
		'albums_total',
		'albums_more',
		'tracks_uris',
		'tracks_total',
		'tracks_more'
	];

	// Loop fields and import from data
	for (var field of fields){
		if (data.hasOwnProperty(field)){
			artist[field] = data[field];
		}
	}

	if (data.images){
		artist.images = [formatImages(artist.images)];
	}

	if (data.followers && data.followers.total !== undefined){
		artist.followers = data.followers.total;
	}

	if (data.bio){
		if (data.bio.content && !artist.biography){
			artist.biography = data.bio.content;
		}
		if (data.bio.links && data.bio.links.link && data.bio.links.link.href && !artist.biography_link){
			artist.biography_link = data.bio.links.link.href;
		}
		if (data.bio.published && !artist.biography_publish_date){
			artist.biography_publish_date = data.bio.published;
		}
	}
    
	if (artist.provider === undefined && artist.uri !== undefined){
		artist.provider = uriSource(artist.uri);
	}

	return artist;
}


/**
 * Format our playlist objects into a universal format
 *
 * @param data obj
 * @return obj
 **/
export let formatPlaylist = function(data){
	var playlist = {}
	var fields = [
		'uri',
		'snapshot_id',
		'provider',
		'type',
		'name',
		'description',
		'images',
		'popularity',
		'followers',
		'added_at',
		'last_modified_date',
		'can_edit',
		'owner',
		'user_uri',
		'tracks_uris',
		'tracks_total',
		'tracks_more'
	];

	// Loop fields and import from data
	for (var field of fields){
		if (data.hasOwnProperty(field)){
			playlist[field] = data[field];
		}
	}

	if (playlist.images && !playlist.images.formatted){
		playlist.images = formatImages(playlist.images);
	}

	if (data.followers && data.followers.total !== undefined){
		playlist.followers = data.followers.total;
	}

	if (data.tracks && data.tracks.total !== undefined){
		playlist.tracks_total = data.tracks.total;
	}

	if (data.owner){
		playlist.owner = {
			id: data.owner.id,
			uri: data.owner.uri,
			name: (data.owner.display_name ? data.owner.display_name : null)
		}
		playlist.user_uri = data.owner.uri;
	}

	// Spotify upgraded their playlists URI to remove user component (Sept 2018)
	playlist.uri = upgradeSpotifyPlaylistUri(playlist.uri);

	if (playlist.provider === undefined && playlist.uri !== undefined){
		playlist.provider = uriSource(playlist.uri);
	}

	return playlist;
}


/**
 * Format a user objects into a universal format
 *
 * @param data obj
 * @return obj
 **/
export let formatUser = function(data){
	var user = {}
	var fields = [
		'id',
		'uri',
		'provider',
		'name',
		'images',
		'followers',
		'playlists_uris',
		'playlists_total',
		'playlists_more'
	];

	// Loop fields and import from data
	for (var field of fields){
		if (data.hasOwnProperty(field)){
			user[field] = data[field];
		}
	}

	if (!user.images && data.image){
		user.images = formatImages(data.image);
	} else if (!user.images && data.avatar){
		user.images = formatImages(data.avatar);
	} else if (user.images && !user.images.formatted){
		user.images = formatImages(user.images);
	}

	if (data.followers && data.followers.total !== undefined){
		user.followers = data.followers.total;
	}
	if (data.realname){
		user.name = data.realname;
	}
	if (data.display_name && !user.name){
		user.name = data.display_name;
	}
	if (data.id && !user.name){
		user.name = data.id;
	}
	if (user.provider === undefined && user.uri !== undefined){
		user.provider = uriSource(user.uri);
	}

	return user;
}


/**
 * Format tracks into our universal format
 *
 * @param data obj
 * @return obj
 **/
export let formatTrack = function(data){
	var track = {}
	var fields = [
		'uri',
		'tlid',
		'provider',
		'name',
		'images',
		'release_date',
		'disc_number',
		'track_number',
		'duration',
		'followers',
		'popularity',
		'userloved',
		'is_explicit',
		'is_local',
		'lyrics',
		'lyrics_path',
		'lyrics_results',
		'artists',	// Array of simple records
		'album'		// Array of simple records
	];

	// Nested track object (eg in spotify playlist)
	if (data && data.track && isObject(data.track)){

		// Copy wrapper's details (if applicable)
		if (data.added_by){
			data.track.added_by = data.added_by;
		}
		if (data.added_at){
			data.track.added_at = data.added_at;
		}
		if (data.tlid){
			data.track.tlid = data.tlid;
		}

		// And now flatten
		data = data.track;
	}

	// Loop fields and import from data
	for (var field of fields){
		if (data.hasOwnProperty(field)){
			track[field] = data[field];
		}
	}

	if (data.followers && data.followers.total){
		track.followers = data.followers.total;
	}

	if (track.duration === undefined && data.duration_ms !== undefined){
		track.duration = data.duration_ms;
	} else if (track.duration === undefined && data.length !== undefined){
		track.duration = data.length;
	}

    if (track.track_number === undefined && data.track_no !== undefined){
    	track.track_number = data.track_no;
    }

    if (track.disc_number === undefined && data.disc_no !== undefined){
    	track.disc_number = data.disc_no;
    }

    if (track.release_date === undefined && data.date !== undefined){
    	track.release_date = data.date;
    }

    if (track.explicit === undefined && data.explicit !== undefined){
    	track.is_explicit = data.explicit;
    }

    // Copy images from albums (if applicable)
    // TOOD: Identify if we stil need this...
    if (data.album && data.album.images){
    	if (track.images === undefined || !track.images.formatted){
    		track.images = formatImages(data.album.images);
    	}
    }

	if (track.provider === undefined && track.uri !== undefined){
		track.provider = uriSource(track.uri);
	}

	return track;
}

/**
 * Format a snapcast client object into a universal format
 *
 * @param data obj
 * @return obj
 **/
export let formatClient = function(data){
	var client = {}
	var fields = [
		'id',
		'connected',
		'name',
		'host_name',
		'volume',
		'mute',
		'latency',
		'power_on_command',
		'power_off_command'
	];

	for (var field of fields){
		if (data.hasOwnProperty(field)){
			client[field] = data[field];
		}
	}

	if (data.config){
		if (client.latency === undefined && data.config.latency !== undefined){
			client.latency = data.config.latency;
		}

		if (!client.name && data.config.name){
			client.name = data.config.name;
		}

		if (data.config.volume){
			if (data.config.volume.percent){
				client.volume = data.config.volume.percent;
			}
			if (data.config.volume.muted){
				client.mute = data.config.volume.muted;
			}
		}
	}

	if (client.name === undefined && data.host && data.host.name){
		client.name = data.host.name;
	}

	return client;
}

/**
 * Format a snapcast client object into a universal format
 *
 * @param data obj
 * @return obj
 **/
export let formatGroup = function(data){
	var group = {}
	var fields = [
		'id',
		'name',
		'mute',
		'stream_id',
		'clients_ids'
	];

	for (var field of fields){
		if (data.hasOwnProperty(field)){
			group[field] = data[field];
		}
	}

	if (group.name === undefined || group.name === ""){
		group.name = 'Group '+data.id.substring(0,3);
	}

	if (group.mute === undefined && data.mute !== undefined){
		group.mute = data.mute;
	}

	return group;
}


/**
 * Collate an object with external references into a fully self-contained object
 * We merge *_uris references (ie tracks_uris) into the main object
 *
 * @param object Obj
 * @param indexes Obj (the relevant core indexes)
 * @return object Obj
 **/
export let collate = function(obj, indexes = {}){

	// First, let's reset this object
	// This is important because by changing this object, we inadvertently
	// change the source object (ie the indexed record), which undoes the
	// efficiencies of a lean index object
	obj = Object.assign({}, obj);

	// Setup empty arrays for the appropriate reference objects
	// This helps create a consistent object structure
	if (obj.artists_uris !== undefined) 		obj.artists = [];
	if (obj.albums_uris !== undefined) 			obj.albums = [];
	if (obj.tracks_uris !== undefined) 			obj.tracks = [];
	if (obj.users_uris !== undefined) 			obj.users = [];
	if (obj.playlists_uris !== undefined) 		obj.playlists = [];
	if (obj.related_artists_uris !== undefined) obj.related_artists = [];
	if (obj.clients_ids !== undefined) 			obj.clients = [];

	if (indexes.artists){
		if (obj.artists_uris){
			for (var uri of obj.artists_uris){
				if (indexes.artists[uri]){
					obj.artists.push(indexes.artists[uri]);
				}
			}
		}
		if (obj.related_artists_uris){
			for (var uri of obj.related_artists_uris){
				if (indexes.artists[uri]){
					obj.related_artists.push(indexes.artists[uri]);
				}
			}
		}
		if (obj.artist_uri){
			if (indexes.artists[obj.artist_uri]){
				obj.artist = indexes.artists[obj.artist_uri];
			}
		}
	}

	if (indexes.albums){
		if (obj.albums_uris){
			for (var uri of obj.albums_uris){
				if (indexes.albums[uri]){
					obj.albums.push(indexes.albums[uri]);
				}
			}
		}
		if (obj.album_uri){
			if (indexes.albums[obj.album_uri]){
				obj.album = indexes.albums[obj.album_uri];
			}
		}
	}

	if (indexes.tracks){
		if (obj.tracks_uris){
			for (var uri of obj.tracks_uris){
				if (indexes.tracks[uri]){
					obj.tracks.push(indexes.tracks[uri]);
				}
			}
		}
		if (obj.track_uri){
			if (indexes.tracks[obj.track_uri]){
				obj.track = indexes.tracks[obj.track_uri];
			}
		}
	}

	if (indexes.users){
		if (obj.users_uris){
			for (var uri of obj.users_uris){
				if (indexes.users[uri]){
					obj.users.push(indexes.users[uri]);
				}
			}
		}
		if (obj.user_uri){
			if (indexes.users[obj.user_uri]){
				obj.user = indexes.users[obj.user_uri];
			}
		}
	}

	if (indexes.playlists){
		if (obj.playlists_uris){
			for (var uri of obj.playlists_uris){
				if (indexes.playlists[uri]){
					obj.playlists.push(indexes.playlists[uri]);
				}
			}
		}
		if (obj.playlist_uri){
			if (indexes.playlists[obj.playlist_uri]){
				obj.playlist = indexes.playlists[obj.playlist_uri];
			}
		}
	}

	if (indexes.clients){
		if (obj.clients_ids){
			for (var id of obj.clients_ids){
				if (indexes.clients[id]){
					obj.clients.push(indexes.clients[id]);
				}
			}
		}
	}

	return obj;
}




/**
 * Figure out a URI's source namespace
 * @param uri = string
 **/
export let uriSource = function(uri){
	if (!uri){
		return false;
	}
    var exploded = uri.split(':');
    return exploded[0]
}

export let sourceIcon = function(uri,source = null){
	if (uri) source = uriSource(uri)
	switch(source){

		case 'local':
		case 'm3u':
			return 'folder'

		case 'gmusic':
			return 'google'

		case 'podcast':
		case 'podcast+file':
		case 'podcast+http':
		case 'podcast+https':
		case 'podcast+itunes':
			return 'podcast'

		case 'tunein':
		case 'somafm':
		case 'dirble':
			return 'microphone'

		default:
			return source
	}
}



/**
 * Get an element from a URI
 * @param element = string, the element we wish to extract
 * @param uri = string
 **/
export let getFromUri = function(element, uri = ""){
    var exploded = uri.split(':');
    var namespace = exploded[0]

    switch (element){
    	case 'mbid':
	        var index = exploded.indexOf('mbid')
	        if (index > -1 ) return exploded[index+1]
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

    	case 'searchtype':
    		if (exploded[0] == "search"){
				return exploded[1];
    		}
    		break

    	case 'searchterm':
    		if (exploded[0] == "search"){
				return exploded[2];
    		}
    		break
    }
    return null
}

/**
 * Identify what kind of asset a URI is (playlist, album, etc)
 *
 * @param uri = string
 * @return string
 **/
export let uriType = function(uri){
	if (!uri) return null;

    var exploded = uri.split(':')

    if (exploded[0] == 'm3u'){
    	return 'playlist'
    }

    if (exploded[0] == 'iris'){
    	switch (exploded[1]){
	    	case 'search':
	    	case 'discover':
	    	case 'browse':
	    		return exploded[1];
	    		break;
	    }
    }

    switch (exploded[1]){
    	case 'track':
    	case 'artist':
    	case 'album':
    	case 'playlist':
    	case 'genre':
    		return exploded[1]
    		break

    	case 'user':
    		if (exploded.length > 3 && exploded[3] == 'playlist'){
    			return 'playlist'
    		}
    		return exploded[1]
    		break
    }
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
 * Digest an array of objects and pull into simple array of one property
 * 
 * @param property = string
 * @param items = Array
 * @return Array
 **/
export let arrayOf = function(property, items){
	let array = [];
	for (var item of items){
		array.push(item[property]);
	}
	return array;
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

	for(var i in list){
		var item = list[i]
		if (item[key] in keyed_list){
			item = Object.assign({}, keyed_list[item[key]], item)
		}
		keyed_list[item[key]] = item;
	}

	for(i in keyed_list){
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

	for (var i in array){
		if (unique.indexOf(array[i]) <= -1){
			unique.push(array[i])
		}
	}

	return unique;
}


/**
 * Apply a partial text search on an array of objects
 *
 * @param field = string (the field we're to search)
 * @param value = string (the value to find)
 * @param array = array of objects to search
 * @param singular = boolean (just return the first result)
 * @return array
 **/
export let applyFilter = function(field, value, array, singular = false){
	var results = [];

	for (var i = 0; i < array.length; i++){
		if (array[i][field] && String(array[i][field]).toLowerCase().includes(String(value).toLowerCase())){
			if (singular){
				return array[i];
			} else {
				results.push(array[i]);
			}
		}
	}

	return results;
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
    for(var i = 0; i < indexes.length; i++){
        if (!previous_index || previous_index == indexes[i]-1){
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
 * @param sort_map = array of value ordering (rather than alphabetical, numerical, etc)
 * @return array
 **/
export let sortItems = function (array, property, reverse = false, sort_map = null){

	function compare(a,b){

		var a_value = a;
		var a_property_split = property.split('.');
		for (var i = 0; i < a_property_split.length; i++){
			if (typeof(a_value[a_property_split[i]]) === 'undefined'){
				a_value = false;
				break;
			} else {
				a_value = a_value[a_property_split[i]];
			}
		}

		var b_value = b;
		var b_property_split = property.split('.');
		for (var i = 0; i < b_property_split.length; i++){
			if (typeof(b_value[b_property_split[i]]) === 'undefined'){
				b_value = false;
				break;
			} else {
				b_value = b_value[b_property_split[i]];
			}
		}

		// Sorting by URI as a reference for sorting by uri source (first component of URI)
		if (property == 'uri'){
			a_value = uriSource(a_value);
			b_value = uriSource(b_value);
		}

		// Map sorting
		// Use the index of the string as a sorting mechanism
		if (sort_map){

			var a_index = sort_map.indexOf(a_value+':');
			var b_index = sort_map.indexOf(b_value+':');
			if (a_index < b_index) return 1;
			if (a_index > b_index) return -1;

		// Boolean sorting
		} else if (typeof(a_value) === 'boolean'){
			if (a_value && !b_value) return -1;
			if (!a_value && b_value) return 1;
			return 0

		// Alphabetic sorting
		} else if (typeof(a_value) === 'string'){
			if (!a_value || !b_value ) return 0;
			if (a_value.toLowerCase() > b_value.toLowerCase()) return 1;
			if (a_value.toLowerCase() < b_value.toLowerCase()) return -1;
			return 0

		// Numeric sorting
		} else {
			if (parseInt(a_value) > parseInt(b_value)) return 1;
			if (parseInt(a_value) < parseInt(b_value)) return -1;
			return 0
		}
	}

	var sorted = Object.assign([], array.sort(compare));
	if (reverse){
		sorted.reverse();
	}
	return sorted;
}

/**
 * Shuffle items in place
 *
 * @param Array items
 * @return Array
 **/
export let shuffle = function(array){
    var j, x, i;
    for (i = array.length - 1; i > 0; i--) {
        j = Math.floor(Math.random() * (i + 1));
        x = array[i];
        array[i] = array[j];
        array[j] = x;
    }
    return array;
}

/**
 * Figure out if a value is a number
 * @param value = mixed
 * @return boolean
 **/
export let isNumeric = function (value){
	return !isNaN(parseFloat(value)) && isFinite(value)
}


/** 
 * Figure out if a value is an object
 * @param value = mixed
 * @return boolean
 **/
export let isObject = function(value){
	return value instanceof Object && value.constructor === Object;
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
	for (var load_queue_key in load_queue){

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


/**
 * Is this app running from the hosted instance?
 * For example the GitHub-hosted UI
 *
 * @param Array hosts = valid hosted domain names
 * @return Boolean
 **/
export let isHosted = function(hosts = ['jaedb.github.io']){
	var hostname = window.location.hostname;
	return hosts.includes(hostname);
}



/**
 * Get indexed record(s) by URI from our asset index
 *
 * @param store = obj
 * @param uris = mixed (array or string)
 * @return array
 **/
export let getIndexedRecords = function(index, uris){
	var records = []

	// Wrap in array, if we've only got one URI
	if (!uris instanceof Array){
		uris = [uris]
	}

	for (var i = 0; i < uris.length; i++){
		if (index.hasOwnProperty(uris[i])){
			records.push(index[uris[i]])
		}
	}

	return records
}


/**
 * Uppercase-ify the first character of a string
 *
 * @param string String
 * @return String
 **/
export let titleCase = function(string){
    return string.charAt(0).toUpperCase() + string.slice(1)
}


/**
 * Scroll to the top of the page
 * Our 'content' is housed in the <main> DOM element
 *
 * @param target String (element ID, optional)
 * @param smooth_scroll Boolean (optional)
 **/
export let scrollTo = function(target = null, smooth_scroll = false){

	var main = document.getElementById('main');

	// Remove our smooth-scroll class
	if (!smooth_scroll){
		main.classList.remove("smooth-scroll");
	}

	// And now scroll to it
	if (target){
		document.getElementById(target).scrollIntoView();
	} else {
		main.scrollTo(0, 0);
	}

	// Now reinstate smooth scroll
	if (!smooth_scroll){
		main.classList.add("smooth-scroll");
	}
}


/**
 * Upgrade one or many Spotify Playlist URIs 
 * This is their new, simplified syntax (September 2018) but they haven't updated it everywhere
 * So we need to manually strip user:abc to keep things consistent
 *
 * @param uris Array|String
 * @return Array|String
 **/
export let upgradeSpotifyPlaylistUris = function(uris){
	var upgraded = [];

	for (var uri of uris){
		if (uri.includes("spotify:user:")){
			uri = uri.replace(/spotify:user:([^:]*?):/i, "spotify:");
		}
		upgraded.push(uri);
	}

	return upgraded;
}

// As above, but for a single URI
export let upgradeSpotifyPlaylistUri = function(uri){
	return upgradeSpotifyPlaylistUris([uri])[0];
}


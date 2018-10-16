
export function set(data){
    return {
        type: 'GOOGLE_SET',
        data: data
    }
}

export function getLibraryArtists(){
	return {
		'type': 'GOOGLE_GET_LIBRARY_ARTISTS'
	}
}
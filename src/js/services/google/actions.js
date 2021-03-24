export function set(data) {
  return {
    type: 'GOOGLE_SET',
    data,
  };
}

export function getLibraryArtists() {
  return {
    type: 'GOOGLE_GET_LIBRARY_ARTISTS',
  };
}

export function clearLibraryArtists() {
  return {
    type: 'GOOGLE_CLEAR_LIBRARY_ARTISTS',
  };
}

export function getLibraryAlbums() {
  return {
    type: 'GOOGLE_GET_LIBRARY_ALBUMS',
  };
}

export function clearLibraryAlbums() {
  return {
    type: 'GOOGLE_CLEAR_LIBRARY_ALBUMS',
  };
}

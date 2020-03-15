
import { removeDuplicates } from '../../util/arrays';

export default function reducer(google = {}, action) {
  switch (action.type) {
    case 'GOOGLE_SET':
      return { ...google, ...action.data };

    case 'GOOGLE_LIBRARY_ARTISTS_LOADED':
      if (google.library_artists) {
        var uris = [...google.library_artists, ...action.uris];
      } else {
        var { uris } = action;
      }
      return { ...google, library_artists: removeDuplicates(uris) };

    case 'GOOGLE_CLEAR_LIBRARY_ARTISTS':
      return { ...google, library_artists: null };

    case 'GOOGLE_LIBRARY_ALBUMS_LOADED':
      if (google.library_albums) {
        var uris = [...google.library_albums, ...action.uris];
      } else {
        var { uris } = action;
      }
      return { ...google, library_albums: removeDuplicates(uris) };

    case 'GOOGLE_CLEAR_LIBRARY_ALBUMS':
      return { ...google, library_albums: null };

    default:
      return google;
  }
}

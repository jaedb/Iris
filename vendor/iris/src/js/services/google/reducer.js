export default function reducer(google = {}, action) {
  switch (action.type) {
    case 'GOOGLE_SET':
      return { ...google, ...action.data };
    default:
      return google;
  }
}

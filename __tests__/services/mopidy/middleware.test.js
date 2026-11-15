import { buildStore } from '../../../src/js/store';
import Mopidy from 'mopidy';
import { getAlbum } from '../../../src/js/services/mopidy/actions';

// The middleware constructs its Mopidy socket on MOPIDY_CONNECT; mock the socket
// so we can serve canned library.lookup responses without a live Mopidy server
jest.mock('mopidy', () => ({
  __esModule: true,
  default: jest.fn(() => ({
    on: jest.fn(),
    close: jest.fn(),
    library: {
      lookup: jest.fn().mockResolvedValue({}),
      getImages: jest.fn().mockResolvedValue({}),
    },
  })),
}));

describe('MOPIDY_GET_ALBUM', () => {
  const uri = 'local:album:md5:66fbea3593ba96a15a9d4188bebab50b';

  const getSocket = () => Mopidy.mock.results[0].value;

  const prepareStore = () => {
    const store = buildStore();
    store.dispatch({ type: 'MOPIDY_CONNECT' });
    store.dispatch({ type: 'MOPIDY_SET', data: { connected: true } });
    return store;
  };

  it('routes the raw mopidy album model through formatAlbum', async () => {
    const store = prepareStore();
    getSocket().library.lookup.mockResolvedValue({
      [uri]: [
        {
          uri: `${uri}:track:2`,
          name: 'Second track',
          length: 2000,
          track_number: 2,
          date: '2023-03-11',
          album: {
            uri,
            name: 'Test album',
            date: '2023-03-11',
          },
          artists: [{ uri: 'local:artist:1', name: 'Test artist' }],
        },
        {
          uri: `${uri}:track:1`,
          name: 'First track',
          length: 1000,
          track_number: 1,
          date: '2023-03-11',
          album: {
            uri,
            name: 'Test album',
            date: '2023-03-11',
          },
          artists: [{ uri: 'local:artist:1', name: 'Test artist' }],
        },
      ],
    });

    store.dispatch(getAlbum(uri));
    await new Promise((resolve) => { setTimeout(resolve, 10); });

    const album = store.getState().core.items[uri];

    // formatAlbum maps the wire's "date" into the "release_date" the view renders
    expect(album.release_date).toBe('2023-03-11');
    expect(album.name).toBe('Test album');
    expect(album.provider).toBe('local');
    expect(album.tracks).toHaveLength(2);
    expect(album.tracks[0].name).toBe('First track');
    expect(album.artists).toHaveLength(1);
    expect(album.artists[0].name).toBe('Test artist');
  });
});

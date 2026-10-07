import React from 'react';
import TestRenderer, { act } from 'react-test-renderer';
import { useDispatch, useSelector } from 'react-redux';
import { GridItem } from '../../src/js/components/GridItem';
import { ListItem } from '../../src/js/components/ListItem';
import * as discogsActions from '../../src/js/services/discogs/actions';
import * as spotifyActions from '../../src/js/services/spotify/actions';
import * as mopidyActions from '../../src/js/services/mopidy/actions';

jest.mock('react-redux', () => ({ useDispatch: jest.fn(), useSelector: jest.fn() }));
jest.mock('react-dnd', () => ({ useDrag: () => [{}, jest.fn()] }));
jest.mock('react-router-dom', () => ({ useNavigate: () => jest.fn(), useLocation: () => ({}) }));
jest.mock('../../src/js/components/Link', () => ({ __esModule: true, default: 'a' }));
jest.mock('../../src/js/components/Thumbnail', () => ({ __esModule: true, default: 'img' }));
jest.mock('../../src/js/components/LinksSentence', () => ({ __esModule: true, default: 'span' }));
jest.mock('../../src/js/components/URILink', () => ({ __esModule: true, default: 'a' }));
jest.mock('../../src/js/components/ContextMenu/ContextMenuTrigger', () => ({ __esModule: true, default: 'span' }));
jest.mock('../../src/js/components/Icon', () => ({ __esModule: true, default: 'span', SourceIcon: 'span' }));
jest.mock('../../src/js/components/Popularity', () => ({ __esModule: true, default: 'span' }));
jest.mock('../../src/js/locale', () => ({ I18n: () => null, i18n: (path) => path }));
jest.mock('../../src/js/services/ui/actions', () => ({}));
jest.mock('../../src/js/services/discogs/actions', () => ({
  getArtistImages: jest.fn((uri, artist) => ({ type: 'TEST_DISCOGS_IMAGES', uri, artist })),
}));
jest.mock('../../src/js/services/spotify/actions', () => ({
  getArtistImages: jest.fn((artist) => ({ type: 'TEST_SPOTIFY_IMAGES', artist })),
}));
jest.mock('../../src/js/services/mopidy/actions', () => ({
  getImages: jest.fn((uris) => ({ type: 'TEST_MOPIDY_IMAGES', uris })),
}));

const artist = { uri: 'local:artist:example', type: 'artist', name: 'Example' };

describe.each([['GridItem', GridItem], ['ListItem', ListItem]])('%s artist images', (name, Component) => {
  let dispatch;
  let renderer;
  const render = (item, access_token = null) => {
    useSelector.mockImplementation((selector) => selector({
      ui: { grid_glow_enabled: false }, spotify: { access_token },
    }));
    act(() => { renderer = TestRenderer.create(<Component item={item} />); });
  };

  beforeEach(() => {
    jest.clearAllMocks();
    dispatch = jest.fn();
    useDispatch.mockReturnValue(dispatch);
  });
  afterEach(() => { if (renderer) act(() => renderer.unmount()); });

  it('requests Discogs images with the artist URI and details when Spotify is unavailable', () => {
    render(artist);
    expect(discogsActions.getArtistImages).toHaveBeenCalledTimes(1);
    expect(discogsActions.getArtistImages).toHaveBeenCalledWith(artist.uri, artist);
    expect(dispatch).toHaveBeenCalledWith({ type: 'TEST_DISCOGS_IMAGES', uri: artist.uri, artist });
    expect(spotifyActions.getArtistImages).not.toHaveBeenCalled();
  });

  it('keeps using Spotify when an access token is available', () => {
    render(artist, 'access-token');
    expect(spotifyActions.getArtistImages).toHaveBeenCalledWith(artist);
    expect(dispatch).toHaveBeenCalledWith({ type: 'TEST_SPOTIFY_IMAGES', artist });
    expect(discogsActions.getArtistImages).not.toHaveBeenCalled();
  });

  it('does not request images when the artist already has them', () => {
    render({ ...artist, images: ['artist.jpg'] });
    expect(dispatch).not.toHaveBeenCalled();
    expect(discogsActions.getArtistImages).not.toHaveBeenCalled();
    expect(spotifyActions.getArtistImages).not.toHaveBeenCalled();
  });

  it('keeps album image requests on Mopidy', () => {
    const album = { uri: 'local:album:example', type: 'album', name: 'Album' };
    render(album);
    expect(mopidyActions.getImages).toHaveBeenCalledWith([album.uri]);
    expect(dispatch).toHaveBeenCalledWith({ type: 'TEST_MOPIDY_IMAGES', uris: [album.uri] });
    expect(discogsActions.getArtistImages).not.toHaveBeenCalled();
  });
});

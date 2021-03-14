import React from 'react';
import { render } from 'enzyme';
import { GridItem } from '../../src/js/components/GridItem';

jest.mock('react-redux', () => ({
  useSelector: jest.fn(),
  useDispatch: () => jest.fn(),
}));
jest.mock('react-router-dom', () => ({
  ...jest.requireActual('react-router-dom'),
  useHistory: () => ({
    location: {
      pathname: 'iris.local:6680/iris/artist/123:abc',
    },
  }),
  useLocation: () => ({
    pathname: 'iris.local:6680/iris/artist/123:abc',
  }),
}));

describe('<GridItem />', () => {

  it('should handle album', () => {
    const item = {
      uri: 'spotify:album:alpha',
      type: 'album',
      name: 'One',
    };
    const dom = render(<GridItem item={item} />);
    expect(dom.find('.grid__item__name').text()).toEqual('One');
    expect(dom.find('.grid__item__secondary__content').length).toBe(1);
  });

  it('should handle artist', () => {
    const item = {
      uri: 'spotify:artist:alpha',
      type: 'artist',
      name: 'Alpha',
      followers: 123,
      albums_uris: ['spotify:album:beta'],
    };
    const dom = render(<GridItem item={item} />);
    console.debug( dom.find('.grid__item__secondary__content') );
    expect(dom.find('.grid__item__name').text()).toEqual('Alpha');
    expect(dom.find('.grid__item__secondary__content').childAt(0).render().text()).toEqual('123 followers');
    expect(dom.find('.grid__item__secondary__content').childAt(1).render().text()).toEqual('1 albums');
  });

  it('should handle playlist', () => {
    const item = {
      uri: 'spotify:playlist:alpha',
      type: 'playlist',
      name: 'One',
      tracks: [
        { uri: 'spotify:track:123' },
        { uri: 'spotify:track:456' },
      ],
    };
    const dom = render(<GridItem item={item} />);
    expect(dom.find('.grid__item__name').text()).toEqual('One');
    expect(dom.find('.grid__item__secondary__content').render().text()).toEqual('2 tracks');
  });
});
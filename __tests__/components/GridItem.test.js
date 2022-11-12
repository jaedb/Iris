import React from 'react';
import { GridItem } from '../../src/js/components/GridItem';
import { render } from '../test-wrapper';
import state from '../state';

jest.mock('react-redux', () => ({
  useSelector: jest.fn(),
  useDispatch: jest.fn(),
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
  const { core: { items } } = state;

  it('should handle album', () => {
    const result = render(
      <GridItem item={items['local:album:md5:66fbea3593ba96a15a9d4188bebab50b']} />
    ).toJSON();
    expect(result).toMatchSnapshot();
  });

  it('should handle artist', () => {
    const result = render(
      <GridItem item={items['local:artist:md5:4f6e4f979e2c40c5e6ad1735804c29bc']} />
    ).toJSON();
    expect(result).toMatchSnapshot();
  });

  it('should handle playlist', () => {
    const result = render(
      <GridItem item={items['m3u:Local%20tester.m3u8']} />
    ).toJSON();
    expect(result).toMatchSnapshot();
  });
});

import React from 'react';
import { BrowserRouter } from 'react-router-dom';
import TestRenderer from 'react-test-renderer';
import { GridItem } from '../../src/js/components/GridItem';
import { core } from '../state';

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
    const result = TestRenderer.create(
      <BrowserRouter>
        <GridItem item={core.items['local:album:md5:66fbea3593ba96a15a9d4188bebab50b']} />
      </BrowserRouter>
    ).toJSON();
    expect(result).toMatchSnapshot();
  });
  it('should handle artist', () => {
    const result = TestRenderer.create(
      <BrowserRouter>
        <GridItem item={core.items['local:artist:md5:4f6e4f979e2c40c5e6ad1735804c29bc']} />
      </BrowserRouter>
    ).toJSON();
    expect(result).toMatchSnapshot();
  });
  it('should handle playlist', () => {
    const result = TestRenderer.create(
      <BrowserRouter>
        <GridItem item={core.items['m3u:Local%20tester.m3u8']} />
      </BrowserRouter>
    ).toJSON();
    expect(result).toMatchSnapshot();
  });
});
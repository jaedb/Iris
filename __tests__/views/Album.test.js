import React from 'react';
import { render } from '../test-wrapper';
import Album from '../../src/js/views/Album';
import { state as mockState } from '../state';

jest.mock('react-dnd', () => ({
  ...jest.requireActual('react-dnd'),
  useDrag: jest.fn(),
  useDrop: jest.fn(),
}));
jest.mock('redux-persist', () => ({
  ...jest.requireActual('redux-persist'),
  persistReducer: jest.fn().mockImplementation((config, reducers) => reducers),
}));
jest.mock('react-redux', () => ({
  useSelector: jest.fn().mockImplementation(func => func(mockState)),
  useDispatch: jest.fn(),
  connect: jest.fn(fn => fn()),
}));
jest.mock('react-router-dom', () => ({
  ...jest.requireActual('react-router-dom'),
  useHistory: () => ({
    location: {
      pathname: 'iris.local:6680/iris/album/local:album:md5:66fbea3593ba96a15a9d4188bebab50b',
    },
  }),
  useLocation: () => ({
    pathname: 'iris.local:6680/iris/album/local:album:md5:66fbea3593ba96a15a9d4188bebab50b',
  }),
}));

describe('<Album />', () => {

  const album = mockState.core.items['local:album:md5:66fbea3593ba96a15a9d4188bebab50b'];

  // Need to rebuild Album to functional component, at which point I'll copy previous
  // snapshot testing approach from other project
  it('should render accurately', () => {
    const result = render(<Album album={album} />).toJSON();
    expect(result).toMatchSnapshot();
  });
});

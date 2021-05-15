import React from 'react';
import TestRenderer from 'react-test-renderer';

import { Album } from '../../src/js/views/Album';
import * as uiActions from '../../src/js/services/ui/actions';
import * as coreActions from '../../src/js/services/core/actions';

const state = require('../state');

jest.mock('react-redux', () => ({
  useSelector: jest.fn(),
  useDispatch: () => jest.fn(),
  connect: () => jest.fn(),
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
  const album = state.core.items['local:album:md5:66fbea3593ba96a15a9d4188bebab50b'];

  // Need to rebuild Album to functional component, at which point I'll copy previous
  // snapshot testing approach from other project
  it.skip('should render accurately', () => {
    const result = TestRenderer.create(
			<Album
				album={album}
				uiActions={uiActions}
				coreActions={coreActions}
			/>
    ).toJSON();
    expect(result).toMatchSnapshot();
  });
});

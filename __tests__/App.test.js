import React from 'react';
import ShallowRenderer from 'react-test-renderer/shallow'
import App from '../src/js/App';

jest.mock('react-redux', () => ({
  useSelector: jest.fn(),
  useDispatch: () => jest.fn(),
  connect: () => jest.fn(),
}));
jest.mock('react-router-dom', () => ({
  ...jest.requireActual('react-router-dom'),
  useHistory: () => ({
    location: {
      pathname: 'iris.local:6680/iris',
    },
  }),
  useLocation: () => ({
    pathname: 'iris.local:6680/iris',
  }),
}));

describe('<App />', () => {
  it('should render', () => {
    const renderer = new ShallowRenderer();
    renderer.render(<App />);
    const result = renderer.getRenderOutput();
    expect(result.type).toEqual('div');
  });
});

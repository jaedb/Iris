import React from 'react';
import { render } from './test-wrapper';
import App from '../src/js/App';
import { state as mockState } from './state';

// jest.mock('react-router-dom', () => ({
//   ...jest.requireActual('react-router-dom'),
//   useNavigate: () => jest.fn(),
//   useHistory: () => ({
//     location: {
//       pathname: 'iris.local:6680/iris',
//     },
//   }),
//   useLocation: () => ({
//     pathname: 'iris.local:6680/iris',
//   }),
// }));
// jest.mock('react-dnd', () => ({
//   ...jest.requireActual('react-dnd'),
//   useDrag: jest.fn(),
//   useDrop: jest.fn(),
// }));
// jest.mock('react-redux', () => ({
//   persistReducer: jest.fn().mockImplementation((config, reducers) => reducers),
//   useSelector: () => jest.fn(fn => fn(mockState)),
//   useDispatch: () => jest.fn(),
//   connect: jest.fn(fn => fn()),
// }));

// TODO
// It seems uncommenting the mocks above causes null state issues
xdescribe('<App />', () => {
  it('should render', () => {
    const result = render(
      <App />,
      { initialState: mockState },
    ).toJSON();
    expect(result).toMatchSnapshot();
  });
});

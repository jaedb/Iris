import React from 'react';
import { render } from '../test-wrapper';
import Album from '../../src/js/views/Album';
import { state as mockState } from '../state';

jest.mock('react-dnd', () => ({
  ...jest.requireActual('react-dnd'),
  useDrag: () => jest.fn(),
  useDrop: () => jest.fn(),
}));
jest.mock('redux-persist', () => ({
  ...jest.requireActual('redux-persist'),
  persistReducer: jest.fn().mockImplementation((config, reducers) => reducers),
}));
jest.mock('react-redux', () => ({
  useSelector: jest.fn().mockImplementation((func) => func(mockState)),
  useDispatch: () => jest.fn(),
  connect: jest.fn(fn => fn()),
}));
jest.mock('react-router-dom', () => ({
  ...jest.requireActual('react-router-dom'),
  useParams: () => ({
    uri: 'bG9jYWw6YWxidW06bWQ1OjY2ZmJlYTM1OTNiYTk2YTE1YTlkNDE4OGJlYmFiNTBi',
  }),
}));

beforeEach(() => {  
  window.IntersectionObserver = jest.fn(() => ({
    observe: jest.fn(),
    unobserve: jest.fn(),
    disconnect: jest.fn(),
  }));
});

describe('<Album />', () => {
  it('should render accurately', () => {
    const result = render(
      <Album />,
      { initialState: mockState },
    ).toJSON();
    expect(result).toMatchSnapshot();
  });
});

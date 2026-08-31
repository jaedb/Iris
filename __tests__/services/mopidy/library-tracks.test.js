import { applyMiddleware, combineReducers, createStore } from 'redux';
import Mopidy from 'mopidy';
import coreReducer from '../../../src/js/services/core/reducer';
import mopidyMiddleware from '../../../src/js/services/mopidy/middleware';
import mopidyReducer from '../../../src/js/services/mopidy/reducer';
import { getLibraryTracks } from '../../../src/js/services/mopidy/actions';
import * as uiActions from '../../../src/js/services/ui/actions';
import uiReducer from '../../../src/js/services/ui/reducer';

jest.mock('mopidy', () => ({
  __esModule: true,
  default: jest.fn(),
}));

const libraryUri = 'local:directory?type=track';
const processKey = 'MOPIDY_GET_LIBRARY_TRACKS';

const makeTrack = (uri) => ({
  uri,
  name: `Track ${uri}`,
  artists: [],
});

const makeSocket = (browseResponse, lookup) => ({
  on: jest.fn(),
  close: jest.fn(),
  library: {
    browse: jest.fn(() => Promise.resolve(browseResponse)),
    lookup,
  },
});

const recordActions = (actions) => () => (next) => (action) => {
  actions.push(action);
  return next(action);
};

const makeStore = (socket, actions = []) => {
  Mopidy.mockImplementation(() => socket);

  const store = createStore(
    combineReducers({
      core: coreReducer,
      mopidy: mopidyReducer,
      ui: uiReducer,
    }),
    {
      core: {
        items: {},
        libraries: {},
      },
      mopidy: {
        connected: false,
        host: 'localhost',
        port: 6680,
        ssl: false,
      },
      ui: {
        allow_reporting: false,
        load_queue: {},
        processes: {},
      },
    },
    applyMiddleware(mopidyMiddleware, recordActions(actions)),
  );

  store.dispatch({ type: 'MOPIDY_CONNECT' });
  store.dispatch({ type: 'MOPIDY_CONNECTED' });

  return store;
};

const waitForRequests = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('Mopidy library track loading', () => {
  beforeEach(() => {
    Mopidy.mockReset();
  });

  it('loads ordered tracks with bounded item commits and accurate progress', async () => {
    const trackUris = Array.from(
      { length: 5005 },
      (_, index) => `test:track:${index}`,
    );
    const browseResponse = trackUris.map((uri) => ({ uri }));
    const lookup = jest.fn(({ uris }) => Promise.resolve(
      uris.reduce((response, uri) => ({
        ...response,
        [uri]: [makeTrack(uri)],
      }), {}),
    ));
    const actions = [];
    const store = makeStore(makeSocket(browseResponse, lookup), actions);

    store.dispatch(uiActions.startLoading(libraryUri, libraryUri));
    store.dispatch(getLibraryTracks(libraryUri));
    await waitForRequests();

    const lookupBatchSizes = lookup.mock.calls.map(([{ uris }]) => uris.length);
    expect(lookupBatchSizes).toHaveLength(51);
    expect(lookupBatchSizes.slice(0, -1).every((size) => size === 100)).toBe(true);
    expect(lookupBatchSizes[lookupBatchSizes.length - 1]).toBe(5);

    const itemActions = actions.filter(({ type }) => type === 'ITEMS_LOADED');
    expect(itemActions.map(({ items }) => items.length)).toEqual([5000, 5]);

    const progressUpdates = actions
      .filter(({ type }) => type === 'UPDATE_PROCESS')
      .map(({ process }) => process);
    expect(progressUpdates[0]).toEqual({ remaining: 5005, total: 5005 });
    expect(progressUpdates[progressUpdates.length - 1]).toEqual({ remaining: 0 });

    const state = store.getState();
    expect(state.ui.processes[processKey].status).toBe('finished');
    expect(state.core.libraries[libraryUri].items_uris).toEqual(trackUris);
    expect(Object.keys(state.core.items)).toHaveLength(trackUris.length);
    expect(new Set(Object.keys(state.core.items)).size).toBe(trackUris.length);
  });

  it('caps commits when missing results cross the item boundary', async () => {
    const trackUris = Array.from(
      { length: 5100 },
      (_, index) => `test:track:${index}`,
    );
    const missingUri = trackUris[4900];
    const browseResponse = trackUris.map((uri) => ({ uri }));
    const lookup = jest.fn(({ uris }) => Promise.resolve(
      uris.reduce((response, uri) => {
        if (uri === missingUri) return response;
        return {
          ...response,
          [uri]: [makeTrack(uri)],
        };
      }, {}),
    ));
    const actions = [];
    const store = makeStore(makeSocket(browseResponse, lookup), actions);

    store.dispatch(getLibraryTracks(libraryUri));
    await waitForRequests();

    const itemActions = actions.filter(({ type }) => type === 'ITEMS_LOADED');
    expect(itemActions.every(({ items }) => items.length <= 5000)).toBe(true);
    expect(itemActions.map(({ items }) => items.length)).toEqual([5000, 99]);

    const progressUpdates = actions
      .filter(({ type }) => type === 'UPDATE_PROCESS')
      .map(({ process }) => process);
    expect(progressUpdates[0]).toEqual({ remaining: 5100, total: 5100 });
    expect(progressUpdates[progressUpdates.length - 1]).toEqual({ remaining: 0 });

    const state = store.getState();
    expect(state.ui.processes[processKey].status).toBe('finished');
    expect(state.core.libraries[libraryUri].items_uris).toEqual(trackUris);
    expect(Object.keys(state.core.items)).toHaveLength(5099);
    expect(trackUris
      .map((uri) => state.core.items[uri])
      .filter(Boolean)
      .map((item) => item.uri)).toEqual(trackUris.filter((uri) => uri !== missingUri));
  });

  it('keeps the browse order while ignoring missing lookup results', async () => {
    const trackUris = [
      'test:track:first',
      'test:track:missing',
      'test:track:last',
    ];
    const browseResponse = trackUris.map((uri) => ({ uri }));
    const lookup = jest.fn(({ uris }) => {
      const response = {};
      response[uris[2]] = [makeTrack(uris[2])];
      response[uris[1]] = [];
      response[uris[0]] = [makeTrack(uris[0])];
      return Promise.resolve(response);
    });
    const store = makeStore(makeSocket(browseResponse, lookup));

    store.dispatch(getLibraryTracks(libraryUri));
    await waitForRequests();

    const { items, libraries } = store.getState().core;
    expect(libraries[libraryUri].items_uris).toEqual(trackUris);
    expect(trackUris.map((uri) => items[uri]).filter(Boolean).map((item) => item.uri))
      .toEqual([trackUris[0], trackUris[2]]);
    expect(items[trackUris[1]]).toBeUndefined();
  });

  it('cancels an in-flight load without requesting or publishing the next batch', async () => {
    const trackUris = Array.from(
      { length: 250 },
      (_, index) => `test:track:${index}`,
    );
    const browseResponse = trackUris.map((uri) => ({ uri }));
    let resolveLookup;
    const lookup = jest.fn(() => new Promise((resolve) => {
      resolveLookup = resolve;
    }));
    const store = makeStore(makeSocket(browseResponse, lookup));

    store.dispatch(uiActions.startLoading(libraryUri, libraryUri));
    store.dispatch(getLibraryTracks(libraryUri));
    await waitForRequests();
    expect(lookup).toHaveBeenCalledTimes(1);

    store.dispatch(uiActions.cancelProcess(processKey));
    resolveLookup({ [trackUris[0]]: [makeTrack(trackUris[0])] });
    await waitForRequests();

    const state = store.getState();
    expect(lookup).toHaveBeenCalledTimes(1);
    expect(state.ui.processes[processKey].status).toBe('cancelled');
    expect(state.ui.load_queue).toEqual({});
    expect(state.core.libraries[libraryUri]).toBeUndefined();
    expect(state.core.items).toEqual({});
  });
});

import { makeSearchResultsSelector } from '../../src/js/util/selectors';
import { getSearchResultKey } from '../../src/js/util/helpers';

jest.mock('../../src/js/locale', () => ({ i18n: (path) => path }));

const term = 'example';
const type = 'albums';
const key = (provider) => getSearchResultKey({ provider, term, type });
const album = { uri: 'local:album:example', name: 'Example', type: 'album' };

describe('search result image enrichment', () => {
  it('merges loaded details by URI, preserving result fields and both inputs', () => {
    const result = Object.freeze({ ...album, images: ['old.jpg'], searchOnly: true });
    const details = Object.freeze({ images: ['loaded.jpg'], name: 'Loaded title' });
    const results = Object.freeze([result]);
    const state = { core: {
      search_results: { [key('local')]: results },
      items: { [album.uri]: details, unrelated: { images: ['wrong.jpg'] } },
    } };
    expect(makeSearchResultsSelector(['local'], term, type)(state)).toEqual([
      { ...result, ...details },
    ]);
    expect(result.images).toEqual(['old.jpg']);
    expect(details).toEqual({ images: ['loaded.jpg'], name: 'Loaded title' });
  });

  it('updates images when item details arrive without replacing search results', () => {
    const selector = makeSearchResultsSelector(['local'], term, type);
    const search_results = { [key('local')]: [album] };
    expect(selector({ core: { search_results, items: {} } })).toEqual([album]);
    expect(selector({ core: { search_results, items: {
      [album.uri]: { images: ['loaded.jpg'] },
    } } })).toEqual([{ ...album, images: ['loaded.jpg'] }]);
  });

  it('preserves provider and result order and skips missing providers', () => {
    const second = { uri: 'local:album:second', name: 'Second' };
    const spotify = { uri: 'spotify:album:example', name: 'Spotify' };
    const state = { core: { items: {}, search_results: {
      [key('local')]: [album, second], [key('spotify')]: [spotify],
    } } };
    expect(makeSearchResultsSelector(['local', 'missing', 'spotify'], term, type)(state))
      .toEqual([album, second, spotify]);
  });

  it('returns no results for an absent search or an empty provider list', () => {
    const state = { core: { items: {}, search_results: {} } };
    expect(makeSearchResultsSelector(['local'], term, type)(state)).toEqual([]);
    expect(makeSearchResultsSelector([], term, type)(state)).toEqual([]);
  });
});

import { pick } from 'lodash';
import React from 'react';
import { connect } from 'react-redux';
import { titleCase, getIndexedRecords } from '../util/helpers';
import { sortItems, indexToArray } from '../util/arrays';
import URILink from './URILink';
import Icon from './Icon';
import AlbumGrid from './AlbumGrid';
import ArtistGrid from './ArtistGrid';
import PlaylistGrid from './PlaylistGrid';
import TrackList from './TrackList';
import LazyLoadListener from './LazyLoadListener';
import { I18n } from '../locale';
import Button from './Button';

const SearchResults = ({
  type,
  query,
  sort,
  sort_reverse,
  uri_schemes_priority,
  all,
  results: rawResults,
}) => {
  const encodedTerm = encodeURIComponent(query.term);
  let results = rawResults;

  if (!results) return null;

  let sort_map = null;
  switch (sort) {
    case 'uri':
      sort_map = uri_schemes_priority;
      break;
    case 'followers':
      // Followers (aka popularlity works in reverse-numerical order)
      // Ie "more popular" is a bigger number
      sort_reverse = !sort_reverse;
      break;
    default:
      break;
  }

  results = sortItems(
    results,
    (type === 'tracks' && sort === 'followers' ? 'popularity' : sort),
    sort_reverse,
    sort_map,
  );

  const resultsCount = results.length;
  if (all && type !== 'tracks' && results.length > 5) {
    results = results.slice(0, 6);
  }

  if (results.length <= 0) return null;

  return (
    <div>
      <h4>
        {!all && (
          <span>
            <URILink uri={`iris:search:all:${encodedTerm}`}>
              <I18n path="search.title" />
            </URILink>
            {' '}
            <Icon type="fontawesome" name="angle-right" />
            {' '}
            <I18n path={`search.${type}.title`} />
          </span>
        )}
        {all && (
          <URILink uri={`iris:search:${type}:${encodedTerm}`}>
            <I18n path={`search.${type}.title`} />
          </URILink>
        )}
      </h4>
      <section className="grid-wrapper">
        {type === 'artists' && <ArtistGrid artists={results} show_source_icon mini={all} />}
        {type === 'albums' && <AlbumGrid albums={results} show_source_icon mini={all} />}
        {type === 'playlists' && <PlaylistGrid playlists={results} show_source_icon mini={all} />}
        {type === 'tracks' && <TrackList tracks={results} uri={`iris:search:${query.type}:${encodedTerm}`} show_source_icon />}
        {/*<LazyLoadListener enabled={this.props.artists_more && spotify_search_enabled} loadMore={loadMore} />*/}

        {resultsCount > results.length && (
          <Button uri={`iris:search:${type}:${encodedTerm}`} debug>
            <I18n path={`search.${type}.more`} count={resultsCount} />
          </Button>
        )}
      </section>
    </div>
  );
};

const getResults = (state, provider, type, query) => {
  const {
    [provider]: {
      search_results: {
        query: resultsQuery,
        [type]: results,
      } = {},
    } = {},
  } = state;

  if (!resultsQuery) return [];
  if (resultsQuery.term !== query.term) return [];
  if (resultsQuery.type !== query.type) return [];

  if (type === 'tracks') {
    return results || [];
  }

  const selectedItems = pick(state.core.items, results);
  return Object.keys(selectedItems).length > 0 ? indexToArray(selectedItems) : [];
};

const mapStateToProps = (state, ownProps) => ({
  uri_schemes_priority: state.ui.uri_schemes_priority || [],
  results: [
    ...getResults(state, 'mopidy', ownProps.type, ownProps.query),
    ...getResults(state, 'spotify', ownProps.type, ownProps.query),
  ],
  sort: state.ui.search_results_sort || 'followers',
  sort_reverse: !!state.ui.search_results_sort_reverse,
});

const mapDispatchToProps = () => ({});

export default connect(mapStateToProps, mapDispatchToProps)(SearchResults);

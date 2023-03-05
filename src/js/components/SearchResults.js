import React from 'react';
import { useSelector } from 'react-redux';
import { useParams } from 'react-router-dom';
import { sortItems } from '../util/arrays';
import URILink from './URILink';
import Icon from './Icon';
import TrackList from './TrackList';
import { Grid } from './Grid';
import { I18n } from '../locale';
import Button from './Button';
import { makeSearchResultsSelector, getSortSelector } from '../util/selectors';

const SearchResults = ({
  type,
  all,
}) => {
  const { term } = useParams();
  const { sortField, sortReverse } = useSelector(
    (state) => getSortSelector(state, 'search_results'),
  );
  const searchResultsSelector = makeSearchResultsSelector(term, type);
  const rawResults = useSelector(searchResultsSelector);
  const encodedTerm = encodeURIComponent(term);
  let results = [...rawResults];

  results = sortItems(
    results,
    (type === 'tracks' && sortField === 'followers' ? 'popularity' : sortField),
    sortReverse,
  );

  const resultsCount = results.length;
  if (all && type !== 'tracks' && results.length > 5) {
    results = results.slice(0, 6);
  }

  if (all && !results.length) return null;

  return (
    <div>
      <h4>
        {!all && (
          <span>
            <URILink uri={`iris:search:all:${encodedTerm}`} uriType="search" unencoded>
              <I18n path="search.title" />
            </URILink>
            {' '}
            <Icon type="fontawesome" name="angle-right" />
            {' '}
            <I18n path={`search.${type}.title`} />
          </span>
        )}
        {all && (
          <URILink uri={`iris:search:${type}:${encodedTerm}`} uriType="search" unencoded>
            <I18n path={`search.${type}.title`} />
          </URILink>
        )}
      </h4>
      {results.length > 0 && (
        <section className="grid-wrapper">
          {type === 'artists' && <Grid items={results} show_source_icon mini={all} />}
          {type === 'albums' && <Grid items={results} show_source_icon mini={all} />}
          {type === 'playlists' && <Grid items={results} show_source_icon mini={all} />}
          {type === 'tracks' && (
            <TrackList
              source={{
                uri: `iris:search:${type}:${encodedTerm}`,
                name: 'Search results',
                type: 'search',
              }}
              tracks={results}
              show_source_icon
            />
          )}
          {/* <LazyLoadListener enabled={this.props.artists_more && spotify_search_enabled} loadMore={loadMore} /> */}

          {resultsCount > results.length && (
            <Button uri={`iris:search:${type}:${encodedTerm}`} uriType="search" unencoded>
              <I18n path={`search.${type}.more`} count={resultsCount} />
            </Button>
          )}
        </section>
      )}
    </div>
  );
};

const AllSearchResults = () => (
  <>
    <div className="search-result-sections cf">
      <section className="search-result-sections__item">
        <div className="inner">
          <SearchResults type="artists" all />
        </div>
      </section>
      <section className="search-result-sections__item">
        <div className="inner">
          <SearchResults type="albums" all />
        </div>
      </section>
      <section className="search-result-sections__item">
        <div className="inner">
          <SearchResults type="playlists" all />
        </div>
      </section>
    </div>
    <SearchResults type="tracks" all />
  </>
);

export {
  SearchResults,
  AllSearchResults,
}
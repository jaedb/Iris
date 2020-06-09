
import React from 'react';
import { connect } from 'react-redux';
import { titleCase, getIndexedRecords } from '../util/helpers';
import { sortItems } from '../util/arrays';
import URILink from './URILink';
import Icon from './Icon';
import AlbumGrid from './AlbumGrid';
import ArtistGrid from './ArtistGrid';
import PlaylistGrid from './PlaylistGrid';
import TrackList from './TrackList';
import LazyLoadListener from './LazyLoadListener';

const SearchResults = ({
  type,
  query,
  loadMore,
  index,
  mopidy_search_results,
  spotify_search_results,
  sort,
  sort_reverse,
  all,
}) => {
  const encodedTerm = encodeURIComponent(query.term);

  let results = [];
  if (mopidy_search_results.query === query.term && mopidy_search_results[type]) {
    results = [
      ...results,
      ...(
        type === 'tracks'
          ? mopidy_search_results[type]
          : getIndexedRecords(index, mopidy_search_results[type])
      ),
    ];
  }

  if (spotify_search_results.query === query.term && spotify_search_results[type]) {
    results = [
      ...results,
      ...(
        type === 'tracks'
          ? spotify_search_results[type]
          : getIndexedRecords(index, spotify_search_results[type])
      ),
    ];
  }

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
              {'Search '}
            </URILink>
            <Icon type="fontawesome" name="angle-right" />
            {` ${titleCase(type)}`}
          </span>
        )}
        {all && (
          <URILink uri={`iris:search:${type}:${encodedTerm}`}>
            {titleCase(type)}
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
          <URILink uri={`iris:search:${type}:${encodedTerm}`} className="button button--default">
            {`All ${type} (${resultsCount})`}
          </URILink>
        )}
      </section>
    </div>
  );
};

const mapStateToProps = (state, ownProps) => ({
  index: state.core[ownProps.type] || [],
  uri_schemes_priority: state.ui.uri_schemes_priority || [],
  mopidy_search_results: state.mopidy.search_results || {},
  spotify_search_results: state.spotify.search_results || {},
  sort: state.ui.search_results_sort || 'followers.total',
  sort_reverse: !!state.ui.search_results_sort_reverse,
});

const mapDispatchToProps = () => ({});

export default connect(mapStateToProps, mapDispatchToProps)(SearchResults);

import React, { useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useDispatch, useSelector } from 'react-redux';
import Header from '../components/Header';
import Icon from '../components/Icon';
import DropdownField from '../components/Fields/DropdownField';
import SearchForm from '../components/Fields/SearchForm';
import { AllSearchResults, SearchResults } from '../components/SearchResults';
import { startSearch } from '../services/core/actions';
import {
  setSort,
  hideContextMenu,
  setWindowTitle,
} from '../services/ui/actions';
import { i18n } from '../locale';
import { getSortSelector } from '../util/selectors';
import useSearchQuery from '../util/useSearchQuery';

const SORT_KEY = 'search_results';

const Search = () => {
  const dispatch = useDispatch();
  const navigate = useNavigate();
  const {
    term,
    type,
    providers,
    allProviders,
    providersString,
  } = useSearchQuery();
  const [sortField, sortReverse] = useSelector(
    (state) => getSortSelector(state, SORT_KEY, 'name'),
  );

  useEffect(() => {
    dispatch(setWindowTitle('Search'));
    $(document).find('.search-form input').trigger('focus');
  }, []);

  useEffect(() => {
    if (term) {
      dispatch(setWindowTitle(i18n('search.title_window', { term: decodeURIComponent(term) })));
      dispatch(startSearch({ term, type, providers }));
    }
  }, [providersString, type, term])

  const onSubmit = (term) => {
    updateSearchQuery(term, providers);
  }

  const updateSearchQuery = (term, providers) => {
    const encodedTerm = encodeURIComponent(term);
    navigate(`/search/${type}/${providers.join(',')}/${encodedTerm || ''}`);
  }

  const onReset = () => navigate('/search');

  const onProvidersChange = (providers) => {
    updateSearchQuery(term, providers)
    dispatch(hideContextMenu());
  }

  const onSortChange = (value) => {
    let reverse = false;
    if (value !== null && sortField === value) {
      reverse = !sortReverse;
    }
    dispatch(setSort(SORT_KEY, value, reverse));
    dispatch(hideContextMenu());
  }

  const sortOptions = [
    { value: 'name', label: i18n('common.name') },
    { value: 'uri', label: i18n('fields.filters.source') },
    { value: 'followers', label: i18n('common.popularity') },
    { value: 'artist', label: i18n('common.artist') },
    { value: 'duration', label: i18n('common.duration') },
  ];

  const providerOptions = allProviders.map((value) => ({ value, label: value}))

  const options = (
    <>
      <DropdownField
        icon="swap_vert"
        name={i18n('common.sort')}
        value={sortField}
        options={sortOptions}
        selected_icon={sortField ? (sortReverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
        handleChange={onSortChange}
        valueAsLabel
      />
      <DropdownField
        icon="cloud"
        name={i18n('search.context_actions.source')}
        value={providers}
        options={providerOptions}
        handleChange={onProvidersChange}
      />
    </>
  );

  return (
    <div className="view search-view">
      <Header options={options}>
        <Icon name="search" type="material" />
      </Header>

      <SearchForm
        key={`search_form_${type}_${term}`}
        term={term}
        onSubmit={onSubmit}
        onReset={onReset}
      />

      <div className="content-wrapper">
        {type != 'all' ? (
          <SearchResults type={type} />
        ) : (
          <AllSearchResults />
        )}
      </div>
    </div>
  );
}

export default Search;

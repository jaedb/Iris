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
  set,
  hideContextMenu,
  setWindowTitle,
} from '../services/ui/actions';
import { i18n } from '../locale';
import { getSortSelector } from '../util/selectors';

const Search = () => {
  const { term, type = 'all' } = useParams();
  const dispatch = useDispatch();
  const navigate = useNavigate();
  const lastQuery = useSelector((state) => state.core?.search_results?.query);
  const { sortField, sortReverse } = useSelector(
    (state) => getSortSelector(state, 'search_results'),
  );

  useEffect(() => {
    dispatch(setWindowTitle('Search'));
    $(document).find('.search-form input').focus();
  }, []);

  useEffect(() => {
    if (term && type && term !== lastQuery?.term) {
      dispatch(setWindowTitle(i18n('search.title_window', { term: decodeURIComponent(term) })));
      dispatch(startSearch({ term, type }));
    }
  }, [term, type])

  const onSubmit = (nextTerm) => {
    const encodedTerm = encodeURIComponent(nextTerm);
    navigate(`/search/${type}/${encodedTerm}`);
  }

  const onReset = () => navigate('/search');

  const onSortChange = (value) => {
    dispatch(set({ uri_schemes_search_enabled: value }));
    dispatch(hideContextMenu());
  }

  const sortOptions = [
    { value: 'followers', label: i18n('common.popularity') },
    { value: 'name', label: i18n('common.name') },
    { value: 'artist', label: i18n('common.artist') },
    { value: 'duration', label: i18n('common.duration') },
  ];

  const options = (
    <DropdownField
      icon="swap_vert"
      name={i18n('common.sort')}
      value={sortField}
      options={sortOptions}
      selected_icon={sortField ? (sortReverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
      handleChange={onSortChange}
      valueAsLabel
    />
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

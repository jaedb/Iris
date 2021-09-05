import React, { useEffect, useState } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Header from '../../components/Header';
import DropdownField from '../../components/Fields/DropdownField';
import FilterField from '../../components/Fields/FilterField';
import Icon from '../../components/Icon';
import { Grid } from '../../components/Grid';
import Loader from '../../components/Loader';
import ErrorMessage from '../../components/ErrorMessage';
import Button from '../../components/Button';
import * as uiActions from '../../services/ui/actions';
import * as coreActions from '../../services/core/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { I18n, i18n } from '../../locale';
import {
  makeItemSelector,
  makeLoadingSelector,
  getSortSelector,
} from '../../util/selectors';
import { sortItems, applyFilter } from '../../util/arrays';
import { decodeUri } from '../../util/format';

const SORT_KEY = 'discover_category';

const DiscoverCategory = ({
  uri,
  category,
  loading,
  playlists: playlistsProp,
  sortField,
  sortReverse,
  spotifyActions: {
    getCategory,
  },
  uiActions: {
    setSort,
    hideContextMenu,
    setWindowTitle,
  },
}) => {
  const [filter, setFilter] = useState('');
  useEffect(() => {
    if (!category) getCategory(uri);
  }, [uri]);

  useEffect(() => {
    if (category) {
      setWindowTitle(category.name);
    } else {
      setWindowTitle(i18n('discover.category.title'));
    }
  }, [category]);

  const refresh = () => {
    hideContextMenu();
    getCategory(uri, { forceRefetch: true });
  }

  const onSortChange = (field) => {
    let reverse = false;
    if (field !== null && sortField === field) {
      reverse = !sortReverse;
    }

    setSort(SORT_KEY, field, reverse);
    hideContextMenu();
  }

  if (loading) {
    return <Loader body loading />;
  }

  if (!category) {
    return (
      <ErrorMessage type="not-found" title="Not found">
        <p>
          <I18n path="errors.uri_not_found" uri={uri} />
        </p>
      </ErrorMessage>
    );
  }

  let playlists = playlistsProp;
  if (sortField) playlists = sortItems(playlists, sortField, sortReverse);
  if (filter && filter !== '') playlists = applyFilter('name', filter, playlists);

  const sort_options = [
    {
      value: null,
      label: i18n('fields.filters.as_loaded'),
    },
    {
      value: 'name',
      label: i18n('fields.filters.name'),
    },
    {
      value: 'tracks',
      label: i18n('fields.filters.tracks'),
    },
  ];

  const options = (
    <>
      <FilterField
        initialValue={filter}
        handleChange={setFilter}
        onSubmit={() => uiActions.hideContextMenu()}
      />
      <DropdownField
        icon="swap_vert"
        name={i18n('fields.sort')}
        value={sortField}
        valueAsLabel
        options={sort_options}
        selected_icon={sortField ? (sortReverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
        handleChange={onSortChange}
      />
      <Button
        noHover
        onClick={refresh}
        tracking={{ category: 'DiscoverCategory', action: 'Refresh' }}
      >
        <Icon name="refresh" />
        <I18n path="actions.refresh" />
      </Button>
    </>
  );

  return (
    <div className="view discover-categories-view">
      <Header uiActions={uiActions} options={options}>
        <Icon name="mood" type="material" />
        {category.name}
      </Header>
      <div className="content-wrapper">
        <section className="grid-wrapper">
          <Grid items={playlists} />
        </section>
      </div>
    </div>
  );
}

const mapStateToProps = (state, ownProps) => {
  const [sortField, sortReverse] = getSortSelector(state, SORT_KEY, null);
  const uri = decodeUri(ownProps.match.params.uri);
  const loadingSelector = makeLoadingSelector([`spotify_category_${uri}`]);
  const categorySelector = makeItemSelector(uri);
  const category = categorySelector(state);
  let playlists = null;
  if (category && category.playlists_uris) {
    const playlistsSelector = makeItemSelector(category.playlists_uris);
    playlists = playlistsSelector(state);
  }

  return {
    uri,
    loading: loadingSelector(state),
    playlists,
    category,
    sortField,
    sortReverse,
  };
};

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  coreActions: bindActionCreators(coreActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverCategory);

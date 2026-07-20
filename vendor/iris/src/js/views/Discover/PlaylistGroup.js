import React, { useEffect, useState } from 'react';
import { useDispatch, useSelector } from 'react-redux';
import { useParams } from 'react-router-dom';
import Header from '../../components/Header';
import DropdownField from '../../components/Fields/DropdownField';
import FilterField from '../../components/Fields/FilterField';
import Icon from '../../components/Icon';
import { Grid } from '../../components/Grid';
import Loader from '../../components/Loader';
import ErrorMessage from '../../components/ErrorMessage';
import Button from '../../components/Button';
import {
  setSort,
  hideContextMenu,
  setWindowTitle,
} from '../../services/ui/actions';
import { loadPlaylistGroup } from '../../services/core/actions';
import { I18n, i18n } from '../../locale';
import {
  makeItemSelector,
  makeLoadingSelector,
  getSortSelector,
} from '../../util/selectors';
import { sortItems, applyFilter } from '../../util/arrays';
import { decodeUri } from '../../util/format';

const SORT_KEY = 'playlist_group';

const PlaylistGroup = () => {
  const dispatch = useDispatch();  
  const { uri: unencodedUri } = useParams();
  const uri = decodeUri(unencodedUri);
  const [sortField, sortReverse] = useSelector((state) => getSortSelector(state, SORT_KEY, null));
  const loading = useSelector(makeLoadingSelector([`playlist_group_${uri}`]));
  const playlistGroup = useSelector(makeItemSelector(uri));
  const playlistsProp = useSelector(makeItemSelector(playlistGroup?.playlists_uris || []));

  const [filter, setFilter] = useState('');
  const { name } = useParams();
  useEffect(() => {
    if (playlistGroup) {
      dispatch(setWindowTitle(playlistGroup.name));
    } else {
      dispatch(setWindowTitle(i18n('discover.category.title')));
    }
  }, [playlistGroup]);

  useEffect(
    () => {
      if (uri) dispatch(loadPlaylistGroup(uri));
    },
    [uri],
  );

  const refresh = () => {
    dispatch(hideContextMenu());
    dispatch(loadPlaylistGroup(uri, { forceRefetch: true }));
  }

  const onSortChange = (field) => {
    let reverse = false;
    if (field !== null && sortField === field) {
      reverse = !sortReverse;
    }

    dispatch(setSort(SORT_KEY, field, reverse));
    dispatch(hideContextMenu());
  }

  if (loading) {
    return <Loader body loading />;
  }

  if (!playlistGroup) {
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
        onSubmit={() => dispatch(hideContextMenu())}
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
        tracking={{ category: 'PlaylistGroup', action: 'Refresh' }}
      >
        <Icon name="refresh" />
        <I18n path="actions.refresh" />
      </Button>
    </>
  );

  return (
    <div className="view discover-categories-view">
      <Header options={options}>
        <Icon name="mood" type="material" />
        {playlistGroup.name || name}
      </Header>
      {playlistGroup?.loading ? (
        <Loader
          progress={{
            remaining: playlistGroup?.playlists_uris?.length,
            total: playlistGroup?.playlists_uris?.length,
          }}
          body
          loading
        />
      ) : (
        <div className="content-wrapper">
          <section className="grid-wrapper">
            <Grid items={playlists} />
          </section>
        </div>
      )}
    </div>
  );
}

export default PlaylistGroup;

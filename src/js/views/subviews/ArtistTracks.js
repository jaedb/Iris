import React, { useState } from 'react';
import { Grid } from '../../components/Grid';
import { useDispatch, useSelector } from 'react-redux';
import TrackList from '../../components/TrackList';
import FilterField from '../../components/Fields/FilterField';
import DropdownField from '../../components/Fields/DropdownField';
import { getSortSelector } from '../../util/selectors';
import { sortItems, applyFilter } from '../../util/arrays';
import { i18n, I18n } from '../../locale';
import {
  hideContextMenu,
  setSort,
} from '../../services/ui/actions';

const SORT_KEY = 'artist_tracks';

export default ({
  artist: {
    uri,
    tracks: tracksProp,
  } = {},
}) => {
  const dispatch = useDispatch();
  const [search, setSearch] = useState('');
  const [sortField, sortReverse] = useSelector(
    (state) => getSortSelector(state, SORT_KEY, null),
  );

  const onSortChange = (field) => {
    let reverse = false;
    if (field !== null && sortField === field) {
      reverse = !sortReverse;
    }

    dispatch(setSort(SORT_KEY, field, reverse));
    dispatch(hideContextMenu());
  }

  let tracks = [...(tracksProp || [])];

  if (sortField && tracks) {
    tracks = sortItems(tracks, sortField, sortReverse);
  }

  if (search && search !== '') {
    tracks = applyFilter('name', search, tracks);
  }

  const sort_options = [
    {
      value: null,
      label: i18n('fields.filters.as_loaded'),
    },
    {
      value: 'name',
      label: i18n('artist.tracks.sort.name'),
    },
    {
      value: 'album',
      label: i18n('artist.tracks.sort.album'),
    },
  ];

  return (
    <div className="body related-artists">
      <section className="list-wrapper no-top-padding">
        <h4 className="no-bottom-margin">
          <I18n path="artist.tracks.title" />
          <div className="actions-wrapper">
            <FilterField
              initialValue={search}
              handleChange={setSearch}
              onSubmit={() => dispatch(hideContextMenu())}
            />
            <DropdownField
              icon="swap_vert"
              name="Sort"
              value={sortField}
              valueAsLabel
              options={sort_options}
              selected_icon={sortField ? (sortReverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
              handleChange={onSortChange}
            />
          </div>
        </h4>
        <TrackList
          className="artist-track-list"
          uri={uri}
          tracks={tracks}
          track_context="artist"
        />
      </section>
    </div>
  );
};

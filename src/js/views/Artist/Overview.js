import React, { useState } from 'react';
import { useDispatch, useSelector } from 'react-redux';
import TrackList from '../../components/TrackList';
import FilterField from '../../components/Fields/FilterField';
import DropdownField from '../../components/Fields/DropdownField';
import { Grid } from '../../components/Grid';
import Button from '../../components/Button';
import RelatedArtists from '../../components/RelatedArtists';
import { i18n, I18n } from '../../locale';
import { sortItems, applyFilter } from '../../util/arrays';
import { encodeUri, formatContext } from '../../util/format';
import { makeSortSelector } from '../../util/selectors';
import {
  hideContextMenu,
  setSort,
} from '../../services/ui/actions';
import Loader from '../../components/Loader';

const SORT_KEY = 'artist_albums';

export default ({
  artist,
  albums: albumsProp,
}) => {
  const dispatch = useDispatch();
  const [search, setSearch] = useState('');
  const [type, setType] = useState(null);
  const sortSelector = makeSortSelector(SORT_KEY, null);
  const [sortField, sortReverse] = useSelector(sortSelector);
  const {
    uri,
    tracks,
    related_artists,
  } = artist;
  let albums = albumsProp;
  const loadingAlbums = artist?.loading && artist.loading === 'albums';

  const onSortChange = (field) => {
    let reverse = false;
    if (field !== null && sortField === field) {
      reverse = !sortReverse;
    }

    dispatch(setSort(SORT_KEY, field, reverse));
    dispatch(hideContextMenu());
  }

  const onSubmit = () => {
    dispatch(hideContextMenu());
  };

  if (sortField && albums) {
    albums = sortItems(albums, sortField, sortReverse);
  }

  if (type && albums) {
    albums = applyFilter('album_type', type, albums);
  }

  if (search && search !== '') {
    albums = applyFilter('name', search, albums);
  }

  const sort_options = [
    {
      value: null,
      label: i18n('artist.albums.sort.default'),
    },
    {
      value: 'name',
      label: i18n('artist.albums.sort.name'),
    },
    {
      value: 'release_date',
      label: i18n('artist.albums.sort.release_date'),
    },
    {
      value: 'tracks',
      label: i18n('artist.albums.sort.track_count'),
    },
  ];

  const filter_type_options = [
    {
      value: null,
      label: i18n('artist.albums.filter.all'),
    },
    {
      value: 'album',
      label: i18n('artist.albums.filter.albums'),
    },
    {
      value: 'single',
      label: i18n('artist.albums.filter.singles'),
    },
  ];

  return (
    <div className="body overview">
      <div className={`top-tracks col col--w${related_artists && related_artists.length > 0 ? '70' : '100'}`}>
        {tracks && <h4><I18n path="artist.overview.top_tracks" /></h4>}
        <div className="list-wrapper">
          <TrackList
            context={formatContext(artist)}
            className="artist-track-list"
            uri={uri}
            tracks={tracks ? tracks.slice(0, 10) : []}
          />
        </div>
      </div>

      <div className="col col--w5" />

      {related_artists && related_artists.length > 0 && (
        <div className="col col--w25 related-artists">
          <h4><I18n path="artist.overview.related_artists.title" /></h4>
          <div className="list-wrapper">
            <RelatedArtists artists={related_artists.slice(0, 6)} />
          </div>
          <Button
            to={`/artist/${encodeUri(uri)}/related-artists`}
            scrollTo="#sub-views-menu"
          >
            <I18n path="artist.overview.related_artists.more" />
          </Button>
        </div>
      )}

      <div className="cf" />

      <div className="albums">
        <h4>
          <I18n path="artist.overview.albums" count={albums ? albums.length : 0} />
          {loadingAlbums && <Loader loading mini />}
          <div className="actions-wrapper">
            <FilterField
              initialValue={search}
              handleChange={setSearch}
              onSubmit={onSubmit}
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
            <DropdownField
              icon="filter_list"
              name="Filter"
              value={type}
              valueAsLabel
              options={filter_type_options}
              handleChange={setType}
            />
          </div>
        </h4>

        <section className="grid-wrapper no-top-padding">
          <Grid items={albums} />
        </section>
      </div>
    </div>
  );
}
